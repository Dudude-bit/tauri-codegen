//! Serde attribute inspection.
//!
//! Every public helper below shares the same underlying walk: iterate
//! each `#[serde(...)]` (or `#[ts(...)]`) attribute on the slice, parse
//! its comma-separated contents into `syn::Meta` items, and inspect them.
//! The shared `for_each_meta_in` combinator lets each helper stay a
//! short match on the one or two metas it cares about, without repeating
//! the boilerplate that used to sit in every function.

use syn::{Expr, Lit, Meta, MetaNameValue};

use crate::utils::{
    to_camel_case, to_kebab_case, to_pascal_case, to_screaming_kebab_case, to_screaming_snake_case,
    to_snake_case,
};

/// Serde container attributes that affect naming / enum representation.
#[derive(Debug, Default)]
pub(super) struct SerdeContainerAttrs {
    /// Value of rename_all attribute (e.g., "camelCase", "snake_case")
    pub rename_all: Option<String>,
    /// Value of tag attribute (e.g., "type")
    pub tag: Option<String>,
    /// Value of content attribute (e.g., "content")
    pub content: Option<String>,
    /// Whether the enum is untagged
    pub untagged: bool,
}

// --- shared walk primitives -------------------------------------------

/// Invoke `f(meta)` for every inner `syn::Meta` inside every
/// `#[<namespace>(...)]` attribute on `attrs`. Non-matching attributes
/// and unparseable bodies are silently skipped. Early exit: returning
/// `true` from the callback stops iteration.
fn for_each_meta_in<F: FnMut(&Meta) -> bool>(attrs: &[syn::Attribute], namespace: &str, mut f: F) {
    for attr in attrs {
        let Meta::List(meta_list) = &attr.meta else {
            continue;
        };
        if !meta_list.path.is_ident(namespace) {
            continue;
        }
        let Ok(nested) = meta_list
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
        else {
            continue;
        };
        for meta in &nested {
            if f(meta) {
                return;
            }
        }
    }
}

/// Extract a `&str` literal from a `NameValue` like `rename = "foo"`.
fn string_value(nv: &MetaNameValue) -> Option<String> {
    match &nv.value {
        Expr::Lit(expr_lit) => match &expr_lit.lit {
            Lit::Str(s) => Some(s.value()),
            _ => None,
        },
        _ => None,
    }
}

// --- public helpers ---------------------------------------------------

/// Get the serde rename value from attributes if present.
pub(super) fn get_serde_rename(attrs: &[syn::Attribute]) -> Option<String> {
    let mut found = None;
    for_each_meta_in(attrs, "serde", |meta| {
        if let Meta::NameValue(nv) = meta {
            if nv.path.is_ident("rename") {
                if let Some(value) = string_value(nv) {
                    found = Some(value);
                    return true;
                }
            }
        }
        false
    });
    found
}

/// Check if a field has `#[ts(optional)]` and validate it's on `Option<T>`.
pub(super) fn has_ts_optional(attrs: &[syn::Attribute], ty: &crate::models::RustType) -> bool {
    let mut result = false;
    for_each_meta_in(attrs, "ts", |meta| {
        if let Meta::Path(path) = meta {
            if path.is_ident("optional") {
                if matches!(ty, crate::models::RustType::Option(_)) {
                    result = true;
                } else {
                    crate::diagnostics::warn(
                        "#[ts(optional)] is only valid on Option<T> fields, ignoring",
                    );
                }
                return true;
            }
        }
        false
    });
    result
}

/// Check if a field has `#[serde(skip)]`.
///
/// Note: we only check for plain `skip`, not `skip_serializing` or
/// `skip_deserializing` — those are directional and a struct used for both
/// input and output should still keep the other direction in TypeScript.
pub(super) fn has_serde_skip(attrs: &[syn::Attribute]) -> bool {
    has_serde_path_flag(attrs, "skip")
}

/// Check if a field has `#[serde(flatten)]`.
pub(super) fn has_serde_flatten(attrs: &[syn::Attribute]) -> bool {
    has_serde_path_flag(attrs, "flatten")
}

/// Check if a container has `#[serde(transparent)]`. Serde requires this to
/// appear on a single-field struct and serializes directly as the inner type.
pub(super) fn has_serde_transparent(attrs: &[syn::Attribute]) -> bool {
    has_serde_path_flag(attrs, "transparent")
}

/// Does `attrs` contain `#[serde(<flag>)]` as a standalone path item?
fn has_serde_path_flag(attrs: &[syn::Attribute], flag: &str) -> bool {
    let mut result = false;
    for_each_meta_in(attrs, "serde", |meta| {
        if let Meta::Path(path) = meta {
            if path.is_ident(flag) {
                result = true;
                return true;
            }
        }
        false
    });
    result
}

/// Check if a field has `#[serde(default)]` or `#[serde(default = "fn")]`.
/// Either form makes the field optional on the wire.
pub(super) fn has_serde_default(attrs: &[syn::Attribute]) -> bool {
    let mut result = false;
    for_each_meta_in(attrs, "serde", |meta| match meta {
        Meta::Path(path) if path.is_ident("default") => {
            result = true;
            true
        }
        Meta::NameValue(nv) if nv.path.is_ident("default") => {
            result = true;
            true
        }
        _ => false,
    });
    result
}

/// Detect `#[serde(skip_serializing_if = "Option::is_none")]` (and any
/// other `::is_none`-suffixed predicate; users often re-export it from
/// their own crate). Makes the TS field optional.
pub(super) fn has_skip_serializing_if_none(attrs: &[syn::Attribute]) -> bool {
    let mut result = false;
    for_each_meta_in(attrs, "serde", |meta| {
        if let Meta::NameValue(nv) = meta {
            if nv.path.is_ident("skip_serializing_if") {
                if let Some(value) = string_value(nv) {
                    if value == "Option::is_none" || value.ends_with("::is_none") {
                        result = true;
                        return true;
                    }
                }
            }
        }
        false
    });
    result
}

/// Parse serde container attributes (rename_all, tag, content, untagged).
pub(super) fn parse_serde_container_attrs(attrs: &[syn::Attribute]) -> SerdeContainerAttrs {
    let mut result = SerdeContainerAttrs::default();
    for_each_meta_in(attrs, "serde", |meta| {
        match meta {
            Meta::NameValue(nv) => {
                if nv.path.is_ident("rename_all") {
                    result.rename_all = string_value(nv);
                } else if nv.path.is_ident("tag") {
                    result.tag = string_value(nv);
                } else if nv.path.is_ident("content") {
                    result.content = string_value(nv);
                }
            }
            Meta::Path(path) => {
                if path.is_ident("untagged") {
                    result.untagged = true;
                }
            }
            _ => {}
        }
        false // keep walking; a container may mix several attrs
    });
    result
}

/// Apply a serde `rename_all` transformation to a single name.
pub(super) fn apply_rename_all(name: &str, rename_all: &Option<String>) -> Option<String> {
    let rule = rename_all.as_ref()?;
    Some(match rule.as_str() {
        "lowercase" => name.to_lowercase(),
        "UPPERCASE" => name.to_uppercase(),
        "camelCase" => to_camel_case(name),
        "snake_case" => to_snake_case(name),
        "SCREAMING_SNAKE_CASE" => to_screaming_snake_case(name),
        "kebab-case" => to_kebab_case(name),
        "SCREAMING-KEBAB-CASE" => to_screaming_kebab_case(name),
        "PascalCase" => to_pascal_case(name),
        unknown => {
            crate::diagnostics::warn(format!(
                "Unknown rename_all convention '{}', using original name. \
                Supported values: lowercase, UPPERCASE, camelCase, snake_case, \
                SCREAMING_SNAKE_CASE, kebab-case, SCREAMING-KEBAB-CASE, PascalCase",
                unknown
            ));
            name.to_string()
        }
    })
}
