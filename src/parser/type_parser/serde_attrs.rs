//! Serde attribute inspection: the leaf helpers that read `#[serde(...)]` and
//! `#[ts(...)]` annotations off a `syn::Attribute` slice. Extracted from the
//! parent `type_parser.rs` so the struct/enum parsing path there stays focused
//! on AST shape rather than attribute parsing.

use syn::{Expr, Lit, Meta};

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

/// Get the serde rename value from attributes if present.
pub(super) fn get_serde_rename(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if let Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("serde") {
                if let Ok(nested) = meta_list.parse_args_with(
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for meta in nested {
                        if let Meta::NameValue(nv) = meta {
                            if nv.path.is_ident("rename") {
                                if let Expr::Lit(expr_lit) = &nv.value {
                                    if let Lit::Str(lit_str) = &expr_lit.lit {
                                        return Some(lit_str.value());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Check if a field has `#[ts(optional)]` and validate it's on `Option<T>`.
pub(super) fn has_ts_optional(attrs: &[syn::Attribute], ty: &crate::models::RustType) -> bool {
    for attr in attrs {
        if let Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("ts") {
                if let Ok(nested) = meta_list.parse_args_with(
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for meta in nested {
                        if let Meta::Path(path) = meta {
                            if path.is_ident("optional") {
                                if matches!(ty, crate::models::RustType::Option(_)) {
                                    return true;
                                } else {
                                    eprintln!(
                                        "Warning: #[ts(optional)] is only valid on Option<T> fields, ignoring"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    false
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

/// Shared predicate: does the attrs slice contain `#[serde(<flag>)]`?
fn has_serde_path_flag(attrs: &[syn::Attribute], flag: &str) -> bool {
    for attr in attrs {
        if let Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("serde") {
                if let Ok(nested) = meta_list.parse_args_with(
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for meta in nested {
                        if let Meta::Path(path) = meta {
                            if path.is_ident(flag) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

/// Parse serde container attributes (rename_all, tag, content, untagged).
pub(super) fn parse_serde_container_attrs(attrs: &[syn::Attribute]) -> SerdeContainerAttrs {
    let mut result = SerdeContainerAttrs::default();

    for attr in attrs {
        if let Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("serde") {
                if let Ok(nested) = meta_list.parse_args_with(
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for meta in nested {
                        match meta {
                            Meta::NameValue(nv) => {
                                if nv.path.is_ident("rename_all") {
                                    if let Expr::Lit(expr_lit) = &nv.value {
                                        if let Lit::Str(lit_str) = &expr_lit.lit {
                                            result.rename_all = Some(lit_str.value());
                                        }
                                    }
                                } else if nv.path.is_ident("tag") {
                                    if let Expr::Lit(expr_lit) = &nv.value {
                                        if let Lit::Str(lit_str) = &expr_lit.lit {
                                            result.tag = Some(lit_str.value());
                                        }
                                    }
                                } else if nv.path.is_ident("content") {
                                    if let Expr::Lit(expr_lit) = &nv.value {
                                        if let Lit::Str(lit_str) = &expr_lit.lit {
                                            result.content = Some(lit_str.value());
                                        }
                                    }
                                }
                            }
                            Meta::Path(path) => {
                                if path.is_ident("untagged") {
                                    result.untagged = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

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
            eprintln!(
                "Warning: Unknown rename_all convention '{}', using original name. \
                Supported values: lowercase, UPPERCASE, camelCase, snake_case, \
                SCREAMING_SNAKE_CASE, kebab-case, SCREAMING-KEBAB-CASE, PascalCase",
                unknown
            );
            name.to_string()
        }
    })
}
