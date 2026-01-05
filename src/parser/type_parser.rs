use crate::models::{EnumVariant, RustEnum, RustStruct, StructField, VariantData, EnumRepresentation};
use crate::utils::{to_camel_case, to_kebab_case, to_screaming_kebab_case, to_screaming_snake_case, to_snake_case};
use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;
use syn::{Fields, Item, ItemEnum, ItemStruct, Expr, Lit, Meta};

use super::type_extractor::parse_type_with_context;

/// Serde container attributes that affect naming
#[derive(Debug, Default)]
struct SerdeContainerAttrs {
    /// Value of rename_all attribute (e.g., "camelCase", "snake_case")
    rename_all: Option<String>,
    /// Value of tag attribute (e.g., "type")
    tag: Option<String>,
    /// Value of content attribute (e.g., "content")
    content: Option<String>,
    /// Whether the enum is untagged
    untagged: bool,
}

/// Parse a Rust source file and extract structs and enums
pub fn parse_types(content: &str, source_file: &Path) -> Result<(Vec<RustStruct>, Vec<RustEnum>)> {
    parse_types_internal(content, source_file, false)
}

/// Parse expanded Rust code (from cargo expand) and extract structs and enums
/// This uses different detection logic since derive macros are already expanded
pub fn parse_types_expanded(content: &str, source_file: &Path) -> Result<(Vec<RustStruct>, Vec<RustEnum>)> {
    parse_types_internal(content, source_file, true)
}

/// Internal parsing function
fn parse_types_internal(content: &str, source_file: &Path, expanded: bool) -> Result<(Vec<RustStruct>, Vec<RustEnum>)> {
    let syntax = syn::parse_file(content)?;
    let mut structs = Vec::new();
    let mut enums = Vec::new();

    // For expanded code, first collect all types that have Serialize/Deserialize impls
    let serializable_types = if expanded {
        collect_serializable_types(&syntax.items)
    } else {
        HashSet::new()
    };

    parse_items(&syntax.items, source_file, expanded, &serializable_types, &mut structs, &mut enums);

    Ok((structs, enums))
}

/// Collect names of all types that have impl Serialize or Deserialize (from cargo expand)
fn collect_serializable_types(items: &[Item]) -> HashSet<String> {
    let mut result = HashSet::new();
    collect_serializable_types_recursive(items, &mut result);
    result
}

fn collect_serializable_types_recursive(items: &[Item], result: &mut HashSet<String>) {
    for item in items {
        match item {
            Item::Impl(item_impl) => {
                check_serde_impl(item_impl, result);
            }
            Item::Mod(module) => {
                if let Some((_, mod_items)) = &module.content {
                    collect_serializable_types_recursive(mod_items, result);
                }
            }
            Item::Const(item_const) => {
                // serde puts impl Serialize/Deserialize inside `const _: () = { impl ... }`
                // We need to parse these blocks too
                if let syn::Expr::Block(expr_block) = &*item_const.expr {
                    for stmt in &expr_block.block.stmts {
                        if let syn::Stmt::Item(Item::Impl(item_impl)) = stmt {
                            check_serde_impl(item_impl, result);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Check if an impl block is for Serialize/Deserialize and extract the type name
fn check_serde_impl(item_impl: &syn::ItemImpl, result: &mut HashSet<String>) {
    if let Some((_, trait_path, _)) = &item_impl.trait_ {
        let trait_name = trait_path.segments.last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        
        if trait_name == "Serialize" || trait_name == "Deserialize" {
            // Extract the type name from self_ty
            if let syn::Type::Path(type_path) = &*item_impl.self_ty {
                if let Some(segment) = type_path.path.segments.last() {
                    result.insert(segment.ident.to_string());
                }
            }
        }
    }
}

/// Recursively parse items from a list
fn parse_items(
    items: &[Item],
    source_file: &Path,
    expanded: bool,
    serializable_types: &HashSet<String>,
    structs: &mut Vec<RustStruct>,
    enums: &mut Vec<RustEnum>,
) {
    for item in items {
        match item {
            Item::Struct(item_struct) => {
                let name = item_struct.ident.to_string();
                let should_include = if expanded {
                    // For expanded code: check impl Serialize/Deserialize OR serde attrs on fields
                    serializable_types.contains(&name) 
                        || is_serializable(&item_struct.attrs) 
                        || has_serde_field_attrs(item_struct)
                } else {
                    is_serializable(&item_struct.attrs)
                };
                
                if should_include {
                    if let Some(s) = parse_struct(item_struct, source_file) {
                        structs.push(s);
                    }
                }
            }
            Item::Enum(item_enum) => {
                let name = item_enum.ident.to_string();
                let should_include = if expanded {
                    // For expanded code: check impl Serialize/Deserialize OR serde attrs on variants
                    serializable_types.contains(&name)
                        || is_serializable(&item_enum.attrs) 
                        || has_serde_variant_attrs(item_enum)
                } else {
                    is_serializable(&item_enum.attrs)
                };
                
                if should_include {
                    if let Some(e) = parse_enum(item_enum, source_file) {
                        enums.push(e);
                    }
                }
            }
            Item::Mod(module) => {
                // Also parse types inside modules (recursively)
                if let Some((_, mod_items)) = &module.content {
                    parse_items(mod_items, source_file, expanded, serializable_types, structs, enums);
                }
            }
            _ => {}
        }
    }
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

    let fields = match &item.fields {
        Fields::Named(named) => named
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
                let final_name = explicit_rename.clone().unwrap_or(field_name);

                // Check for #[ts(optional)] attribute
                let use_optional = has_ts_optional(&field.attrs, &field_type);

                // Check for #[serde(flatten)] attribute
                let is_flatten = has_serde_flatten(&field.attrs);

                Some(StructField {
                    name: final_name,
                    ty: field_type,
                    has_explicit_rename: explicit_rename.is_some(),
                    use_optional,
                    is_flatten,
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
                    ty: parse_type_with_context(&field.ty, &generic_params),
                    has_explicit_rename: false,
                    use_optional: false,
                    is_flatten: false,
                })
                .collect()
        }
        Fields::Unit => Vec::new(),
    };

    Some(RustStruct {
        name,
        generics,
        fields,
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
            let final_name = explicit_rename.clone()
                .or_else(|| apply_rename_all(&variant_name, &container_attrs.rename_all))
                .unwrap_or(variant_name.clone());
            let has_explicit_rename = explicit_rename.is_some() || container_attrs.rename_all.is_some();

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

/// Get the serde rename value from attributes if present
fn get_serde_rename(attrs: &[syn::Attribute]) -> Option<String> {
    for attr in attrs {
        if let Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("serde") {
                // Parse the serde attribute arguments properly
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

/// Check if a field has #[ts(optional)] attribute and validate it's on Option<T>
fn has_ts_optional(attrs: &[syn::Attribute], ty: &crate::models::RustType) -> bool {
    for attr in attrs {
        if let Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("ts") {
                if let Ok(nested) = meta_list.parse_args_with(
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for meta in nested {
                        if let Meta::Path(path) = meta {
                            if path.is_ident("optional") {
                                // Validate that the type is Option<T>
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

/// Check if a field has #[serde(skip)] attribute
/// Note: We only check for `skip`, not `skip_serializing` or `skip_deserializing`
/// because those are directional - a struct might be used for both input and output,
/// and directional skips should still appear in TypeScript for the other direction.
fn has_serde_skip(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if let Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("serde") {
                if let Ok(nested) = meta_list.parse_args_with(
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for meta in nested {
                        if let Meta::Path(path) = meta {
                            if path.is_ident("skip") {
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

/// Check if a field has #[serde(flatten)] attribute
fn has_serde_flatten(attrs: &[syn::Attribute]) -> bool {
    for attr in attrs {
        if let Meta::List(meta_list) = &attr.meta {
            if meta_list.path.is_ident("serde") {
                if let Ok(nested) = meta_list.parse_args_with(
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                ) {
                    for meta in nested {
                        if let Meta::Path(path) = meta {
                            if path.is_ident("flatten") {
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

/// Parse serde container attributes (rename_all, etc.)
fn parse_serde_container_attrs(attrs: &[syn::Attribute]) -> SerdeContainerAttrs {
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

/// Apply rename_all transformation to a name
fn apply_rename_all(name: &str, rename_all: &Option<String>) -> Option<String> {
    let rule = rename_all.as_ref()?;
    Some(match rule.as_str() {
        "lowercase" => name.to_lowercase(),
        "UPPERCASE" => name.to_uppercase(),
        "camelCase" => to_camel_case(name),
        "snake_case" => to_snake_case(name),
        "SCREAMING_SNAKE_CASE" => to_screaming_snake_case(name),
        "kebab-case" => to_kebab_case(name),
        "SCREAMING-KEBAB-CASE" => to_screaming_kebab_case(name),
        "PascalCase" => name.to_string(), // Usually already PascalCase in Rust
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



#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::RustType;
    use std::path::PathBuf;

    fn test_path() -> PathBuf {
        PathBuf::from("test.rs")
    }

    #[test]
    fn test_parse_ts_optional_attribute() {
        let code = r#"
            #[derive(Serialize)]
            pub struct Config {
                #[ts(optional)]
                pub volume: Option<f32>,
                pub name: Option<String>,
            }
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1);
        let config = &structs[0];

        assert_eq!(config.fields[0].name, "volume");
        assert!(config.fields[0].use_optional, "Option field with #[ts(optional)] should have use_optional=true");
        
        assert_eq!(config.fields[1].name, "name");
        assert!(!config.fields[1].use_optional, "Option field without attribute should have use_optional=false");
    }

    #[test]
    fn test_parse_ts_optional_ignored_on_non_option() {
        // This should print a warning but not fail
        let code = r#"
            #[derive(Serialize)]
            pub struct Config {
                #[ts(optional)]
                pub count: i32,
            }
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1);
        let config = &structs[0];

        assert_eq!(config.fields[0].name, "count");
        assert!(!config.fields[0].use_optional, "Non-Option field should ignore ts(optional)");
    }

    #[test]
    fn test_parse_ts_optional_on_struct_variant() {
        let code = r#"
            #[derive(Serialize)]
            pub enum Settings {
                Network {
                    #[ts(optional)]
                    proxy: Option<String>,
                    port: i32,
                }
            }
        "#;

        let (_, enums) = parse_types(code, &test_path()).unwrap();
        assert_eq!(enums.len(), 1);
        let settings = &enums[0];
        
        match &settings.variants[0].data {
            VariantData::Struct(fields) => {
                assert_eq!(fields[0].name, "proxy");
                assert!(fields[0].use_optional);
                assert_eq!(fields[1].name, "port");
                assert!(!fields[1].use_optional);
            },
            _ => panic!("Expected Struct variant"),
        }
    }



    #[test]
    fn test_parse_simple_struct() {
        let code = r#"
            #[derive(Serialize)]
            pub struct User {
                pub id: i32,
                pub name: String,
            }
        "#;

        let (structs, enums) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1);
        assert_eq!(enums.len(), 0);

        let user = &structs[0];
        assert_eq!(user.name, "User");
        assert_eq!(user.fields.len(), 2);
        assert_eq!(user.fields[0].name, "id");
        assert_eq!(user.fields[1].name, "name");
    }

    #[test]
    fn test_parse_struct_with_generics() {
        let code = r#"
            #[derive(Serialize)]
            pub struct Wrapper<T> {
                pub data: T,
                pub count: i32,
            }
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1);

        let wrapper = &structs[0];
        assert_eq!(wrapper.name, "Wrapper");
        assert_eq!(wrapper.generics, vec!["T"]);
        assert_eq!(wrapper.fields.len(), 2);

        // data field should be Generic(T)
        match &wrapper.fields[0].ty {
            RustType::Generic(name) => assert_eq!(name, "T"),
            other => panic!("Expected Generic(T), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_struct_with_multiple_generics() {
        let code = r#"
            #[derive(Serialize, Deserialize)]
            pub struct Pair<K, V> {
                pub key: K,
                pub value: V,
            }
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1);

        let pair = &structs[0];
        assert_eq!(pair.generics, vec!["K", "V"]);
    }

    #[test]
    fn test_parse_tuple_struct() {
        let code = r#"
            #[derive(Serialize)]
            pub struct Point(i32, i32);
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1);

        let point = &structs[0];
        assert_eq!(point.name, "Point");
        assert_eq!(point.fields.len(), 2);
        assert_eq!(point.fields[0].name, "field0");
        assert_eq!(point.fields[1].name, "field1");
    }

    #[test]
    fn test_parse_simple_enum() {
        let code = r#"
            #[derive(Serialize)]
            pub enum Status {
                Active,
                Inactive,
                Pending,
            }
        "#;

        let (structs, enums) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 0);
        assert_eq!(enums.len(), 1);

        let status = &enums[0];
        assert_eq!(status.name, "Status");
        assert_eq!(status.variants.len(), 3);
        assert_eq!(status.variants[0].name, "Active");
        assert_eq!(status.variants[1].name, "Inactive");
        assert_eq!(status.variants[2].name, "Pending");

        // All should be unit variants
        for variant in &status.variants {
            match &variant.data {
                VariantData::Unit => {}
                other => panic!("Expected Unit, got {:?}", other),
            }
        }
    }

    #[test]
    fn test_parse_enum_with_tuple_data() {
        let code = r#"
            #[derive(Serialize)]
            pub enum Message {
                Text(String),
                Number(i32),
                Pair(String, i32),
            }
        "#;

        let (_, enums) = parse_types(code, &test_path()).unwrap();
        assert_eq!(enums.len(), 1);

        let message = &enums[0];
        assert_eq!(message.variants.len(), 3);

        match &message.variants[0].data {
            VariantData::Tuple(types) => {
                assert_eq!(types.len(), 1);
            }
            other => panic!("Expected Tuple, got {:?}", other),
        }

        match &message.variants[2].data {
            VariantData::Tuple(types) => {
                assert_eq!(types.len(), 2);
            }
            other => panic!("Expected Tuple with 2 elements, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_enum_with_struct_variant() {
        let code = r#"
            #[derive(Serialize)]
            pub enum UserRole {
                Admin { permissions: Vec<String> },
                User,
                Guest,
            }
        "#;

        let (_, enums) = parse_types(code, &test_path()).unwrap();
        assert_eq!(enums.len(), 1);

        let role = &enums[0];

        match &role.variants[0].data {
            VariantData::Struct(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "permissions");
            }
            other => panic!("Expected Struct variant, got {:?}", other),
        }

        match &role.variants[1].data {
            VariantData::Unit => {}
            other => panic!("Expected Unit, got {:?}", other),
        }
    }

    #[test]
    fn test_serde_rename_field() {
        let code = r#"
            #[derive(Serialize)]
            pub struct User {
                #[serde(rename = "userId")]
                pub id: i32,
                pub name: String,
            }
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1);

        let user = &structs[0];
        assert_eq!(user.fields[0].name, "userId");
        assert!(user.fields[0].has_explicit_rename, "Field with serde rename should have has_explicit_rename = true");
        assert_eq!(user.fields[1].name, "name");
        assert!(!user.fields[1].has_explicit_rename, "Field without serde rename should have has_explicit_rename = false");
    }

    #[test]
    fn test_serde_rename_variant() {
        let code = r#"
            #[derive(Serialize)]
            pub enum Status {
                #[serde(rename = "ACTIVE")]
                Active,
                #[serde(rename = "INACTIVE")]
                Inactive,
            }
        "#;

        let (_, enums) = parse_types(code, &test_path()).unwrap();
        assert_eq!(enums.len(), 1);

        let status = &enums[0];
        assert_eq!(status.variants[0].name, "ACTIVE");
        assert!(status.variants[0].has_explicit_rename, "Variant with serde rename should have has_explicit_rename = true");
        assert_eq!(status.variants[1].name, "INACTIVE");
        assert!(status.variants[1].has_explicit_rename, "Variant with serde rename should have has_explicit_rename = true");
    }

    #[test]
    fn test_ignore_non_serializable() {
        let code = r#"
            pub struct NotExported {
                pub id: i32,
            }

            #[derive(Debug)]
            pub struct AlsoNotExported {
                pub name: String,
            }

            #[derive(Serialize)]
            pub struct Exported {
                pub data: String,
            }
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].name, "Exported");
    }

    #[test]
    fn test_parse_types_in_mod() {
        let code = r#"
            mod types {
                #[derive(Serialize)]
                pub struct InnerType {
                    pub value: i32,
                }

                #[derive(Deserialize)]
                pub enum InnerEnum {
                    A,
                    B,
                }
            }
        "#;

        let (structs, enums) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1);
        assert_eq!(enums.len(), 1);
        assert_eq!(structs[0].name, "InnerType");
        assert_eq!(enums[0].name, "InnerEnum");
    }

    #[test]
    fn test_deserialize_also_works() {
        let code = r#"
            #[derive(Deserialize)]
            pub struct Request {
                pub data: String,
            }
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].name, "Request");
    }

    #[test]
    fn test_source_file_is_set() {
        let code = r#"
            #[derive(Serialize)]
            pub struct Test {
                pub value: i32,
            }
        "#;

        let path = PathBuf::from("src/types.rs");
        let (structs, _) = parse_types(code, &path).unwrap();
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].source_file, path);
    }

    #[test]
    fn test_complex_field_types() {
        let code = r#"
            #[derive(Serialize)]
            pub struct Complex {
                pub items: Vec<Item>,
                pub optional: Option<String>,
                pub map: HashMap<String, i32>,
                pub nested: Vec<Option<User>>,
            }
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1);
        assert_eq!(structs[0].fields.len(), 4);

        match &structs[0].fields[0].ty {
            RustType::Vec(_) => {}
            other => panic!("Expected Vec, got {:?}", other),
        }

        match &structs[0].fields[1].ty {
            RustType::Option(_) => {}
            other => panic!("Expected Option, got {:?}", other),
        }

        match &structs[0].fields[2].ty {
            RustType::HashMap { .. } => {}
            other => panic!("Expected HashMap, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_expanded_code_with_serde_attrs() {
        let code = r#"
            pub mod types {
                pub struct AuthResponse {
                    #[serde(rename = "accessToken")]
                    pub access_token: ::std::string::String,
                    #[serde(rename = "refreshToken")]
                    pub refresh_token: ::std::string::String,
                }
            }
        "#;
        
        let (structs, _) = super::parse_types_expanded(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1, "Should find AuthResponse struct");
        assert_eq!(structs[0].name, "AuthResponse");
    }

    #[test]
    fn test_parse_expanded_without_derive_but_with_serde_field_attrs() {
        // This simulates cargo expand output where derive is already expanded
        let code = r#"
            pub struct User {
                #[serde(rename = "userId")]
                pub user_id: i32,
                pub name: String,
            }
        "#;
        
        let (structs, _) = super::parse_types_expanded(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1, "Should find User struct via serde field attrs");
        assert_eq!(structs[0].name, "User");
    }

    #[test]
    fn test_parse_types_regular_ignores_without_derive() {
        // Regular parse_types should NOT find structs without derive
        let code = r#"
            pub struct User {
                #[serde(rename = "userId")]
                pub user_id: i32,
            }
        "#;

        let (structs, _) = super::parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 0, "Regular parse should not find struct without derive");
    }

    #[test]
    fn test_serde_skip_fields_are_excluded() {
        let code = r#"
            #[derive(Serialize)]
            pub struct User {
                pub id: i32,
                pub name: String,
                #[serde(skip)]
                pub internal_cache: Vec<u8>,
                #[serde(skip_serializing)]
                pub password_hash: String,
                #[serde(skip_deserializing)]
                pub computed_field: i32,
            }
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 1);
        // Only #[serde(skip)] should be excluded
        // skip_serializing and skip_deserializing are directional and should be kept
        assert_eq!(structs[0].fields.len(), 4);
        assert_eq!(structs[0].fields[0].name, "id");
        assert_eq!(structs[0].fields[1].name, "name");
        assert_eq!(structs[0].fields[2].name, "password_hash");
        assert_eq!(structs[0].fields[3].name, "computed_field");
    }

    #[test]
    fn test_serde_skip_in_enum_variant_struct() {
        let code = r#"
            #[derive(Serialize)]
            pub enum Event {
                Login {
                    user_id: i32,
                    #[serde(skip)]
                    internal_token: String,
                },
            }
        "#;

        let (_, enums) = parse_types(code, &test_path()).unwrap();
        assert_eq!(enums.len(), 1);

        match &enums[0].variants[0].data {
            crate::models::VariantData::Struct(fields) => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].name, "user_id");
            }
            other => panic!("Expected Struct variant, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_serde_flatten() {
        let code = r#"
            #[derive(Serialize)]
            pub struct Address {
                pub city: String,
                pub country: String,
            }

            #[derive(Serialize)]
            pub struct User {
                pub name: String,
                #[serde(flatten)]
                pub address: Address,
            }
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        assert_eq!(structs.len(), 2);

        let user = structs.iter().find(|s| s.name == "User").unwrap();
        assert_eq!(user.fields.len(), 2);

        assert_eq!(user.fields[0].name, "name");
        assert!(!user.fields[0].is_flatten, "Regular field should not be flattened");

        assert_eq!(user.fields[1].name, "address");
        assert!(user.fields[1].is_flatten, "Field with #[serde(flatten)] should be flattened");
    }

    #[test]
    fn test_parse_multiple_serde_flatten() {
        let code = r#"
            #[derive(Serialize)]
            pub struct Metadata {
                pub created_at: String,
            }

            #[derive(Serialize)]
            pub struct Address {
                pub city: String,
            }

            #[derive(Serialize)]
            pub struct User {
                pub name: String,
                #[serde(flatten)]
                pub address: Address,
                #[serde(flatten)]
                pub meta: Metadata,
            }
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        let user = structs.iter().find(|s| s.name == "User").unwrap();

        assert_eq!(user.fields.len(), 3);
        assert!(!user.fields[0].is_flatten); // name
        assert!(user.fields[1].is_flatten);  // address
        assert!(user.fields[2].is_flatten);  // meta
    }

    #[test]
    fn test_parse_serde_flatten_with_other_attrs() {
        let code = r#"
            #[derive(Serialize)]
            pub struct Inner {
                pub value: i32,
            }

            #[derive(Serialize)]
            pub struct Outer {
                #[serde(rename = "customName")]
                pub name: String,
                #[serde(flatten)]
                pub inner: Inner,
            }
        "#;

        let (structs, _) = parse_types(code, &test_path()).unwrap();
        let outer = structs.iter().find(|s| s.name == "Outer").unwrap();

        assert_eq!(outer.fields[0].name, "customName");
        assert!(outer.fields[0].has_explicit_rename);
        assert!(!outer.fields[0].is_flatten);

        assert!(outer.fields[1].is_flatten);
    }
}
