use super::{command::parse_type, EnumVariant, RustEnum, RustStruct, StructField, VariantData};
use anyhow::Result;
use std::path::Path;
use syn::{Fields, Item, ItemEnum, ItemStruct};

/// Parse a Rust source file and extract structs and enums
pub fn parse_types(content: &str, _source_file: &Path) -> Result<(Vec<RustStruct>, Vec<RustEnum>)> {
    let syntax = syn::parse_file(content)?;
    let mut structs = Vec::new();
    let mut enums = Vec::new();

    for item in syntax.items {
        match item {
            Item::Struct(item_struct) => {
                if is_serializable(&item_struct.attrs) {
                    if let Some(s) = parse_struct(&item_struct) {
                        structs.push(s);
                    }
                }
            }
            Item::Enum(item_enum) => {
                if is_serializable(&item_enum.attrs) {
                    if let Some(e) = parse_enum(&item_enum) {
                        enums.push(e);
                    }
                }
            }
            Item::Mod(module) => {
                // Also parse types inside modules
                if let Some((_, items)) = module.content {
                    for mod_item in items {
                        match mod_item {
                            Item::Struct(item_struct) => {
                                if is_serializable(&item_struct.attrs) {
                                    if let Some(s) = parse_struct(&item_struct) {
                                        structs.push(s);
                                    }
                                }
                            }
                            Item::Enum(item_enum) => {
                                if is_serializable(&item_enum.attrs) {
                                    if let Some(e) = parse_enum(&item_enum) {
                                        enums.push(e);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok((structs, enums))
}

/// Check if a type has Serialize or Deserialize derive attribute
/// This indicates the type is meant for serialization and should be exported
fn is_serializable(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if let syn::Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("derive") {
                let tokens = meta_list.tokens.to_string();
                if tokens.contains("Serialize") || tokens.contains("Deserialize") {
                    return true;
                }
            }
        }
    }
    false
}

/// Parse a struct into our RustStruct representation
fn parse_struct(item: &ItemStruct) -> Option<RustStruct> {
    let name = item.ident.to_string();

    let fields = match &item.fields {
        Fields::Named(named) => named
            .named
            .iter()
            .filter_map(|field| {
                let field_name = field.ident.as_ref()?.to_string();
                let field_type = parse_type(&field.ty);

                // Check for serde rename attribute
                let final_name = get_serde_rename(&field.attrs).unwrap_or(field_name);

                Some(StructField {
                    name: final_name,
                    ty: field_type,
                })
            })
            .collect(),
        Fields::Unnamed(unnamed) => {
            // Tuple struct - use numbered field names
            unnamed
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, field)| StructField {
                    name: format!("field{}", i),
                    ty: parse_type(&field.ty),
                })
                .collect()
        }
        Fields::Unit => Vec::new(),
    };

    Some(RustStruct { name, fields })
}

/// Parse an enum into our RustEnum representation
fn parse_enum(item: &ItemEnum) -> Option<RustEnum> {
    let name = item.ident.to_string();

    let variants = item
        .variants
        .iter()
        .map(|variant| {
            let variant_name = variant.ident.to_string();

            // Check for serde rename attribute
            let final_name = get_serde_rename(&variant.attrs).unwrap_or(variant_name);

            let data = match &variant.fields {
                Fields::Unit => VariantData::Unit,
                Fields::Unnamed(unnamed) => {
                    let types = unnamed.unnamed.iter().map(|f| parse_type(&f.ty)).collect();
                    VariantData::Tuple(types)
                }
                Fields::Named(named) => {
                    let fields = named
                        .named
                        .iter()
                        .filter_map(|field| {
                            let field_name = field.ident.as_ref()?.to_string();
                            let final_name = get_serde_rename(&field.attrs).unwrap_or(field_name);
                            Some(StructField {
                                name: final_name,
                                ty: parse_type(&field.ty),
                            })
                        })
                        .collect();
                    VariantData::Struct(fields)
                }
            };

            EnumVariant {
                name: final_name,
                data,
            }
        })
        .collect();

    Some(RustEnum { name, variants })
}

/// Get the serde rename value from attributes if present
fn get_serde_rename(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if let syn::Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("serde") {
                let tokens = meta_list.tokens.to_string();
                // Look for rename = "..."
                if let Some(start) = tokens.find("rename") {
                    let rest = &tokens[start..];
                    if let Some(eq_pos) = rest.find('=') {
                        let after_eq = rest[eq_pos + 1..].trim();
                        // Extract the string value
                        if let Some(quote_start) = after_eq.find('"') {
                            let after_quote = &after_eq[quote_start + 1..];
                            if let Some(quote_end) = after_quote.find('"') {
                                return Some(after_quote[..quote_end].to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}
