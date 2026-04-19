mod expanded;
mod serde_attrs;

use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;
use syn::{Fields, Item, ItemEnum, ItemStruct, Meta};

use crate::models::{
    EnumRepresentation, EnumVariant, RustEnum, RustStruct, RustTypeAlias, StructField, VariantData,
};

use super::type_extractor::parse_type_with_context;
use crate::models::StructShape;
use expanded::collect_serializable_types;
use serde_attrs::{
    apply_rename_all, get_serde_rename, has_serde_default, has_serde_flatten, has_serde_skip,
    has_serde_transparent, has_skip_serializing_if_none, has_ts_optional,
    parse_serde_container_attrs,
};

/// Parsed types from a Rust file.
#[derive(Debug, Default, Clone)]
pub struct ParsedTypes {
    pub structs: Vec<RustStruct>,
    pub enums: Vec<RustEnum>,
    pub aliases: Vec<RustTypeAlias>,
}

/// Switches that control the discovery strategy.
///
/// Two orthogonal choices collapse into four presets:
///
/// * **Where the input came from** — a plain Rust source file (where
///   `#[derive(Serialize)]` is still an attribute text) versus the
///   output of `cargo expand` (where the derive has already been
///   replaced by an explicit `impl Serialize for Foo` block).
/// * **Whether to filter to types that carry a serde derive** — the
///   pipeline collects everything it can reach so the resolver has a
///   complete view; tests usually want filtering on so they can
///   exercise it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ParseOptions {
    /// Input is `cargo expand` output rather than raw source.
    pub expanded: bool,
    /// Include every struct/enum/alias, not just ones with serde derives.
    pub include_all: bool,
}

impl ParseOptions {
    /// Source file, only types with `#[derive(Serialize)]` / `Deserialize`.
    /// The typical "show me what the user wrote" mode.
    pub const SOURCE: Self = Self {
        expanded: false,
        include_all: false,
    };
    /// Source file, every struct/enum/alias regardless of serde attrs.
    /// Used by the reachable-type walker — if a command signature
    /// names a type, we want its definition even without a derive.
    pub const SOURCE_ALL: Self = Self {
        expanded: false,
        include_all: true,
    };
    /// `cargo expand` output, only types with a serde impl.
    pub const EXPANDED: Self = Self {
        expanded: true,
        include_all: false,
    };
    /// `cargo expand` output, every type. Used by the pipeline to
    /// register macro-generated types that aren't visible in source.
    pub const EXPANDED_ALL: Self = Self {
        expanded: true,
        include_all: true,
    };
}

/// Parse a Rust source buffer and return the types it defines.
///
/// Replaces the previous four `parse_types*` wrappers (`parse_types`,
/// `parse_types_with_aliases`, `parse_types_expanded`,
/// `parse_types_expanded_with_aliases`) — the distinctions now live in
/// `ParseOptions`. Destructure `ParsedTypes` when you only care about a
/// subset of the output:
/// `let ParsedTypes { structs, .. } = parse_types(code, path, ParseOptions::SOURCE)?;`
pub fn parse_types(
    content: &str,
    source_file: &Path,
    options: ParseOptions,
) -> Result<ParsedTypes> {
    parse_types_internal(content, source_file, options.expanded, options.include_all)
}

fn parse_types_internal(
    content: &str,
    source_file: &Path,
    expanded: bool,
    include_all: bool,
) -> Result<ParsedTypes> {
    let syntax = syn::parse_file(content)?;
    let mut parsed = ParsedTypes::default();

    // For expanded code, first collect all types that have Serialize/Deserialize impls
    let serializable_types = if expanded {
        collect_serializable_types(&syntax.items)
    } else {
        HashSet::new()
    };

    parse_items(
        &syntax.items,
        source_file,
        expanded,
        include_all,
        &serializable_types,
        &mut parsed,
    );

    Ok(parsed)
}

/// Recursively parse items from a list
fn parse_items(
    items: &[Item],
    source_file: &Path,
    expanded: bool,
    include_all: bool,
    serializable_types: &HashSet<String>,
    parsed: &mut ParsedTypes,
) {
    for item in items {
        match item {
            Item::Struct(item_struct) => {
                let name = item_struct.ident.to_string();
                let should_include = if include_all {
                    true
                } else if expanded {
                    // For expanded code: check impl Serialize/Deserialize OR serde attrs on fields
                    serializable_types.contains(&name)
                        || is_serializable(&item_struct.attrs)
                        || has_serde_field_attrs(item_struct)
                } else {
                    is_serializable(&item_struct.attrs)
                };

                if should_include {
                    if let Some(s) = parse_struct(item_struct, source_file) {
                        parsed.structs.push(s);
                    }
                }
            }
            Item::Enum(item_enum) => {
                let name = item_enum.ident.to_string();
                let should_include = if include_all {
                    true
                } else if expanded {
                    // For expanded code: check impl Serialize/Deserialize OR serde attrs on variants
                    serializable_types.contains(&name)
                        || is_serializable(&item_enum.attrs)
                        || has_serde_variant_attrs(item_enum)
                } else {
                    is_serializable(&item_enum.attrs)
                };

                if should_include {
                    if let Some(e) = parse_enum(item_enum, source_file) {
                        parsed.enums.push(e);
                    }
                }
            }
            Item::Type(item_type) => {
                if let Some(alias) = parse_alias(item_type, source_file) {
                    parsed.aliases.push(alias);
                }
            }
            Item::Mod(module) => {
                // Also parse types inside modules (recursively)
                if let Some((_, mod_items)) = &module.content {
                    parse_items(
                        mod_items,
                        source_file,
                        expanded,
                        include_all,
                        serializable_types,
                        parsed,
                    );
                }
            }
            _ => {}
        }
    }
}

fn parse_alias(item_type: &syn::ItemType, source_file: &Path) -> Option<RustTypeAlias> {
    let name = item_type.ident.to_string();
    let generics: Vec<String> = item_type
        .generics
        .params
        .iter()
        .filter_map(|param| match param {
            syn::GenericParam::Type(ty) => Some(ty.ident.to_string()),
            _ => None,
        })
        .collect();

    let generic_params: HashSet<String> = generics.iter().cloned().collect();
    let target = parse_type_with_context(&item_type.ty, &generic_params);

    Some(RustTypeAlias {
        name,
        generics,
        target,
        source_file: source_file.to_path_buf(),
    })
}

/// Check if a type has Serialize or Deserialize derive attribute
/// This indicates the type is meant for serialization and should be exported
fn is_serializable(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if let Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("derive") {
                // Parse the derive macro arguments properly
                if let Ok(nested) = meta_list.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                ) {
                    for path in nested {
                        if let Some(ident) = path.get_ident() {
                            let name = ident.to_string();
                            if name == "Serialize" || name == "Deserialize" {
                                return true;
                            }
                        }
                        // Also check for fully qualified paths like serde::Serialize
                        if let Some(last) = path.segments.last() {
                            let name = last.ident.to_string();
                            if name == "Serialize" || name == "Deserialize" {
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

/// Check if a struct has serde attributes on its fields (for expanded code)
/// In cargo expand output, derive macros are already expanded, so we check for
/// #[serde(...)] attributes on fields instead
fn has_serde_field_attrs(item: &ItemStruct) -> bool {
    if let Fields::Named(named) = &item.fields {
        for field in &named.named {
            for attr in &field.attrs {
                if attr.path().is_ident("serde") {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if an enum has serde attributes on variants or variant fields
fn has_serde_variant_attrs(item: &ItemEnum) -> bool {
    for variant in &item.variants {
        // Check variant attrs
        for attr in &variant.attrs {
            if attr.path().is_ident("serde") {
                return true;
            }
        }
        // Check variant field attrs
        match &variant.fields {
            Fields::Named(named) => {
                for field in &named.named {
                    for attr in &field.attrs {
                        if attr.path().is_ident("serde") {
                            return true;
                        }
                    }
                }
            }
            Fields::Unnamed(unnamed) => {
                for field in &unnamed.unnamed {
                    for attr in &field.attrs {
                        if attr.path().is_ident("serde") {
                            return true;
                        }
                    }
                }
            }
            Fields::Unit => {}
        }
    }
    false
}

/// Parse a struct into our RustStruct representation
fn parse_struct(item: &ItemStruct, source_file: &Path) -> Option<RustStruct> {
    let name = item.ident.to_string();

    // Parse container-level serde attributes (like rename_all)
    let container_attrs = parse_serde_container_attrs(&item.attrs);

    // Extract generic type parameters
    let generics: Vec<String> = item
        .generics
        .params
        .iter()
        .filter_map(|param| {
            if let syn::GenericParam::Type(type_param) = param {
                Some(type_param.ident.to_string())
            } else {
                None
            }
        })
        .collect();

    // Create a set for efficient lookup when parsing field types
    let generic_params: HashSet<String> = generics.iter().cloned().collect();

    let transparent = has_serde_transparent(&item.attrs);

    let (fields, mut shape): (Vec<StructField>, StructShape) = match &item.fields {
        Fields::Named(named) => {
            let fields: Vec<StructField> = named
                .named
                .iter()
                .filter_map(|field| {
                    // Skip fields with #[serde(skip)] or similar
                    if has_serde_skip(&field.attrs) {
                        return None;
                    }

                    let field_name = field.ident.as_ref()?.to_string();
                    let field_type = parse_type_with_context(&field.ty, &generic_params);

                    // Check for serde rename attribute
                    let explicit_rename = get_serde_rename(&field.attrs);
                    let final_name = explicit_rename
                        .clone()
                        .or_else(|| apply_rename_all(&field_name, &container_attrs.rename_all))
                        .unwrap_or(field_name);
                    let has_rename =
                        explicit_rename.is_some() || container_attrs.rename_all.is_some();

                    // #[ts(optional)] OR #[serde(default)] OR
                    // #[serde(skip_serializing_if = "Option::is_none")] on
                    // an Option<T> — each of these makes the field optional
                    // in the JSON serde actually emits.
                    let use_optional = has_ts_optional(&field.attrs, &field_type)
                        || (matches!(field_type, crate::models::RustType::Option(_))
                            && (has_serde_default(&field.attrs)
                                || has_skip_serializing_if_none(&field.attrs)));

                    // Check for #[serde(flatten)] attribute
                    let is_flatten = has_serde_flatten(&field.attrs);

                    Some(StructField {
                        name: final_name,
                        ty: field_type,
                        has_explicit_rename: has_rename,
                        use_optional,
                        is_flatten,
                    })
                })
                .collect();
            (fields, StructShape::Named)
        }
        Fields::Unnamed(unnamed) => {
            // Tuple struct — name the positions with their index so the
            // generator can choose the shape. A single unnamed field is a
            // newtype and serializes transparently; multi-field tuples
            // serialize as JSON arrays.
            let fields: Vec<StructField> = unnamed
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, field)| StructField {
                    name: format!("{}", i),
                    ty: parse_type_with_context(&field.ty, &generic_params),
                    has_explicit_rename: false,
                    use_optional: false,
                    is_flatten: false,
                })
                .collect();
            let shape = if fields.len() == 1 {
                StructShape::Newtype
            } else {
                StructShape::Tuple
            };
            (fields, shape)
        }
        Fields::Unit => (Vec::new(), StructShape::Unit),
    };

    // `#[serde(transparent)]` forces the single-field form, whether the
    // original was named or tuple.
    if transparent && fields.len() == 1 {
        shape = StructShape::Newtype;
    }

    Some(RustStruct {
        name,
        generics,
        fields,
        shape,
        source_file: source_file.to_path_buf(),
    })
}

/// Parse an enum into our RustEnum representation
fn parse_enum(item: &ItemEnum, source_file: &Path) -> Option<RustEnum> {
    let name = item.ident.to_string();

    // Extract generic type parameters
    let generics: Vec<String> = item
        .generics
        .params
        .iter()
        .filter_map(|param| {
            if let syn::GenericParam::Type(type_param) = param {
                Some(type_param.ident.to_string())
            } else {
                None
            }
        })
        .collect();

    // Create a set for efficient lookup when parsing field types
    let generic_params: HashSet<String> = generics.iter().cloned().collect();

    // Parse container-level serde attributes (like rename_all)
    let container_attrs = parse_serde_container_attrs(&item.attrs);

    let representation = if container_attrs.untagged {
        EnumRepresentation::Untagged
    } else if let Some(tag) = &container_attrs.tag {
        if let Some(content) = &container_attrs.content {
            EnumRepresentation::Adjacent {
                tag: tag.clone(),
                content: content.clone(),
            }
        } else {
            EnumRepresentation::Internal { tag: tag.clone() }
        }
    } else {
        EnumRepresentation::External
    };

    let variants = item
        .variants
        .iter()
        .map(|variant| {
            let variant_name = variant.ident.to_string();

            // Check for serde rename attribute on variant
            let explicit_rename = get_serde_rename(&variant.attrs);
            let final_name = explicit_rename
                .clone()
                .or_else(|| apply_rename_all(&variant_name, &container_attrs.rename_all))
                .unwrap_or(variant_name.clone());
            let has_explicit_rename =
                explicit_rename.is_some() || container_attrs.rename_all.is_some();

            let data = match &variant.fields {
                Fields::Unit => VariantData::Unit,
                Fields::Unnamed(unnamed) => {
                    let types = unnamed
                        .unnamed
                        .iter()
                        .map(|f| parse_type_with_context(&f.ty, &generic_params))
                        .collect();
                    VariantData::Tuple(types)
                }
                Fields::Named(named) => {
                    let fields = named
                        .named
                        .iter()
                        .filter_map(|field| {
                            // Skip fields with #[serde(skip)] or similar
                            if has_serde_skip(&field.attrs) {
                                return None;
                            }

                            let field_name = field.ident.as_ref()?.to_string();
                            let field_type = parse_type_with_context(&field.ty, &generic_params);
                            let explicit_rename = get_serde_rename(&field.attrs);
                            let final_name = explicit_rename.clone().unwrap_or(field_name);
                            let use_optional = has_ts_optional(&field.attrs, &field_type);
                            let is_flatten = has_serde_flatten(&field.attrs);
                            Some(StructField {
                                name: final_name,
                                ty: field_type,
                                has_explicit_rename: explicit_rename.is_some(),
                                use_optional,
                                is_flatten,
                            })
                        })
                        .collect();
                    VariantData::Struct(fields)
                }
            };

            EnumVariant {
                name: final_name,
                data,
                has_explicit_rename,
            }
        })
        .collect();

    Some(RustEnum {
        name,
        generics,
        variants,
        source_file: source_file.to_path_buf(),
        representation,
    })
}

#[cfg(test)]
mod tests;
