use crate::models::RustType;
use std::collections::HashSet;
use syn::{GenericArgument, PathArguments, Type};

/// Parse a Rust type into our RustType representation (without generic context)
pub fn parse_type(ty: &Type) -> RustType {
    parse_type_with_context(ty, &HashSet::new())
}

/// Parse a Rust type with known generic parameters from the parent struct/enum
pub fn parse_type_with_context(ty: &Type, generic_params: &HashSet<String>) -> RustType {
    match ty {
        Type::Path(type_path) => {
            if let Some(segment) = type_path.path.segments.last() {
                let name = segment.ident.to_string();

                // First check if it's a known generic parameter from the context
                if generic_params.contains(&name) {
                    return RustType::Generic(name);
                }

                match name.as_str() {
                    // Primitive types
                    "String" | "str" | "char" => RustType::Primitive("String".to_string()),
                    "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => {
                        RustType::Primitive(name.clone())
                    }
                    "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => {
                        RustType::Primitive(name.clone())
                    }
                    "f32" | "f64" => RustType::Primitive(name.clone()),
                    "bool" => RustType::Primitive("bool".to_string()),

                    // Well-known external types (serialized as strings)
                    "DateTime" | "NaiveDateTime" | "NaiveDate" | "NaiveTime" // chrono
                    | "OffsetDateTime" | "PrimitiveDateTime" | "Date" | "Time" // time crate
                    | "Uuid" // uuid
                    | "Decimal" | "BigDecimal" // decimal
                    | "PathBuf" | "Path" // std::path
                    | "Url" // url
                    | "IpAddr" | "Ipv4Addr" | "Ipv6Addr" // std::net
                    => RustType::Primitive(name.clone()),

                    // Duration (serialized as number in milliseconds/seconds)
                    "Duration" => RustType::Primitive("Duration".to_string()),

                    // serde_json::Value (any JSON)
                    "Value" => RustType::Primitive("Value".to_string()),

                    // Bytes
                    "Bytes" => RustType::Primitive("Bytes".to_string()),

                    // Generic types
                    "Vec" => {
                        if let Some(inner) = extract_single_generic(&segment.arguments) {
                            RustType::Vec(Box::new(parse_type_with_context(&inner, generic_params)))
                        } else {
                            RustType::Unknown("Vec<?>".to_string())
                        }
                    }
                    "Option" => {
                        if let Some(inner) = extract_single_generic(&segment.arguments) {
                            RustType::Option(Box::new(parse_type_with_context(
                                &inner,
                                generic_params,
                            )))
                        } else {
                            RustType::Unknown("Option<?>".to_string())
                        }
                    }
                    "Result" => {
                        if let Some(inner) = extract_single_generic(&segment.arguments) {
                            RustType::Result(Box::new(parse_type_with_context(
                                &inner,
                                generic_params,
                            )))
                        } else {
                            RustType::Unknown("Result<?>".to_string())
                        }
                    }
                    "HashMap" | "BTreeMap" => {
                        if let Some((key, value)) = extract_two_generics(&segment.arguments) {
                            RustType::HashMap {
                                key: Box::new(parse_type_with_context(&key, generic_params)),
                                value: Box::new(parse_type_with_context(&value, generic_params)),
                            }
                        } else {
                            RustType::Unknown("HashMap<?, ?>".to_string())
                        }
                    }

                    // Custom types (not a known generic param)
                    _ => {
                        // Reconstruct full path for custom types
                        let full_name = type_path
                            .path
                            .segments
                            .iter()
                            .map(|s| s.ident.to_string())
                            .collect::<Vec<_>>()
                            .join("::");
                        RustType::Custom(full_name)
                    }
                }
            } else {
                RustType::Unknown("unknown path".to_string())
            }
        }

        Type::Tuple(tuple) => {
            if tuple.elems.is_empty() {
                RustType::Unit
            } else {
                let types = tuple
                    .elems
                    .iter()
                    .map(|t| parse_type_with_context(t, generic_params))
                    .collect();
                RustType::Tuple(types)
            }
        }

        Type::Reference(reference) => {
            // For references, we parse the inner type
            parse_type_with_context(&reference.elem, generic_params)
        }

        Type::Slice(slice) => {
            // Treat slices like Vec
            RustType::Vec(Box::new(parse_type_with_context(
                &slice.elem,
                generic_params,
            )))
        }

        _ => RustType::Unknown(format!("{:?}", ty)),
    }
}

/// Extract a single generic type argument (for Vec<T>, Option<T>)
fn extract_single_generic(args: &PathArguments) -> Option<Type> {
    if let PathArguments::AngleBracketed(angle) = args {
        if let Some(GenericArgument::Type(ty)) = angle.args.first() {
            return Some(ty.clone());
        }
    }
    None
}

/// Extract two generic type arguments (for HashMap<K, V>)
fn extract_two_generics(args: &PathArguments) -> Option<(Type, Type)> {
    if let PathArguments::AngleBracketed(angle) = args {
        let mut iter = angle.args.iter();
        if let (Some(GenericArgument::Type(first)), Some(GenericArgument::Type(second))) =
            (iter.next(), iter.next())
        {
            return Some((first.clone(), second.clone()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to parse a type string into syn::Type
    fn parse_type_str(s: &str) -> Type {
        syn::parse_str(s).expect("Failed to parse type")
    }

    #[test]
    fn test_parse_primitive_string() {
        let ty = parse_type_str("String");
        match parse_type(&ty) {
            RustType::Primitive(name) => assert_eq!(name, "String"),
            other => panic!("Expected Primitive, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_primitive_str() {
        let ty = parse_type_str("str");
        match parse_type(&ty) {
            RustType::Primitive(name) => assert_eq!(name, "String"),
            other => panic!("Expected Primitive(String), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_primitive_integers() {
        for int_type in ["i8", "i16", "i32", "i64", "i128", "isize"] {
            let ty = parse_type_str(int_type);
            match parse_type(&ty) {
                RustType::Primitive(name) => assert_eq!(name, int_type),
                other => panic!("Expected Primitive({}), got {:?}", int_type, other),
            }
        }

        for uint_type in ["u8", "u16", "u32", "u64", "u128", "usize"] {
            let ty = parse_type_str(uint_type);
            match parse_type(&ty) {
                RustType::Primitive(name) => assert_eq!(name, uint_type),
                other => panic!("Expected Primitive({}), got {:?}", uint_type, other),
            }
        }
    }

    #[test]
    fn test_parse_primitive_floats() {
        for float_type in ["f32", "f64"] {
            let ty = parse_type_str(float_type);
            match parse_type(&ty) {
                RustType::Primitive(name) => assert_eq!(name, float_type),
                other => panic!("Expected Primitive({}), got {:?}", float_type, other),
            }
        }
    }

    #[test]
    fn test_parse_primitive_bool() {
        let ty = parse_type_str("bool");
        match parse_type(&ty) {
            RustType::Primitive(name) => assert_eq!(name, "bool"),
            other => panic!("Expected Primitive(bool), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_vec_type() {
        let ty = parse_type_str("Vec<String>");
        match parse_type(&ty) {
            RustType::Vec(inner) => match *inner {
                RustType::Primitive(name) => assert_eq!(name, "String"),
                other => panic!("Expected Primitive inside Vec, got {:?}", other),
            },
            other => panic!("Expected Vec, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_vec_nested() {
        let ty = parse_type_str("Vec<Vec<i32>>");
        match parse_type(&ty) {
            RustType::Vec(inner) => match *inner {
                RustType::Vec(inner2) => match *inner2 {
                    RustType::Primitive(name) => assert_eq!(name, "i32"),
                    other => panic!("Expected Primitive(i32), got {:?}", other),
                },
                other => panic!("Expected Vec inside Vec, got {:?}", other),
            },
            other => panic!("Expected Vec, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_option_type() {
        let ty = parse_type_str("Option<String>");
        match parse_type(&ty) {
            RustType::Option(inner) => match *inner {
                RustType::Primitive(name) => assert_eq!(name, "String"),
                other => panic!("Expected Primitive inside Option, got {:?}", other),
            },
            other => panic!("Expected Option, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_option_custom_type() {
        let ty = parse_type_str("Option<User>");
        match parse_type(&ty) {
            RustType::Option(inner) => match *inner {
                RustType::Custom(name) => assert_eq!(name, "User"),
                other => panic!("Expected Custom inside Option, got {:?}", other),
            },
            other => panic!("Expected Option, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_result_type() {
        let ty = parse_type_str("Result<User, String>");
        match parse_type(&ty) {
            RustType::Result(ok) => match *ok {
                RustType::Custom(name) => assert_eq!(name, "User"),
                other => panic!("Expected Custom inside Result, got {:?}", other),
            },
            other => panic!("Expected Result, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_hashmap_type() {
        let ty = parse_type_str("HashMap<String, i32>");
        match parse_type(&ty) {
            RustType::HashMap { key, value } => {
                match *key {
                    RustType::Primitive(name) => assert_eq!(name, "String"),
                    other => panic!("Expected Primitive key, got {:?}", other),
                }
                match *value {
                    RustType::Primitive(name) => assert_eq!(name, "i32"),
                    other => panic!("Expected Primitive value, got {:?}", other),
                }
            }
            other => panic!("Expected HashMap, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_btreemap_type() {
        let ty = parse_type_str("BTreeMap<String, User>");
        match parse_type(&ty) {
            RustType::HashMap { key, value } => {
                match *key {
                    RustType::Primitive(name) => assert_eq!(name, "String"),
                    other => panic!("Expected Primitive key, got {:?}", other),
                }
                match *value {
                    RustType::Custom(name) => assert_eq!(name, "User"),
                    other => panic!("Expected Custom value, got {:?}", other),
                }
            }
            other => panic!("Expected HashMap (from BTreeMap), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_tuple_type() {
        let ty = parse_type_str("(i32, String, bool)");
        match parse_type(&ty) {
            RustType::Tuple(types) => {
                assert_eq!(types.len(), 3);
                match &types[0] {
                    RustType::Primitive(name) => assert_eq!(name, "i32"),
                    other => panic!("Expected Primitive(i32), got {:?}", other),
                }
                match &types[1] {
                    RustType::Primitive(name) => assert_eq!(name, "String"),
                    other => panic!("Expected Primitive(String), got {:?}", other),
                }
                match &types[2] {
                    RustType::Primitive(name) => assert_eq!(name, "bool"),
                    other => panic!("Expected Primitive(bool), got {:?}", other),
                }
            }
            other => panic!("Expected Tuple, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_unit_type() {
        let ty = parse_type_str("()");
        match parse_type(&ty) {
            RustType::Unit => {}
            other => panic!("Expected Unit, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_custom_type() {
        let ty = parse_type_str("User");
        match parse_type(&ty) {
            RustType::Custom(name) => assert_eq!(name, "User"),
            other => panic!("Expected Custom, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_nested_generics() {
        let ty = parse_type_str("Vec<Option<User>>");
        match parse_type(&ty) {
            RustType::Vec(inner) => match *inner {
                RustType::Option(inner2) => match *inner2 {
                    RustType::Custom(name) => assert_eq!(name, "User"),
                    other => panic!("Expected Custom, got {:?}", other),
                },
                other => panic!("Expected Option, got {:?}", other),
            },
            other => panic!("Expected Vec, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_result_with_vec() {
        let ty = parse_type_str("Result<Vec<Item>, String>");
        match parse_type(&ty) {
            RustType::Result(ok) => match *ok {
                RustType::Vec(inner) => match *inner {
                    RustType::Custom(name) => assert_eq!(name, "Item"),
                    other => panic!("Expected Custom, got {:?}", other),
                },
                other => panic!("Expected Vec, got {:?}", other),
            },
            other => panic!("Expected Result, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_generic_param_in_context() {
        let ty = parse_type_str("T");
        let mut generics = HashSet::new();
        generics.insert("T".to_string());

        match parse_type_with_context(&ty, &generics) {
            RustType::Generic(name) => assert_eq!(name, "T"),
            other => panic!("Expected Generic, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_generic_param_not_in_context() {
        let ty = parse_type_str("T");
        let generics = HashSet::new();

        match parse_type_with_context(&ty, &generics) {
            RustType::Custom(name) => assert_eq!(name, "T"),
            other => panic!("Expected Custom (unknown generic), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_external_types() {
        for ext_type in [
            "DateTime",
            "NaiveDateTime",
            "NaiveDate",
            "Uuid",
            "PathBuf",
            "Url",
            "IpAddr",
            "Duration",
        ] {
            let ty = parse_type_str(ext_type);
            match parse_type(&ty) {
                RustType::Primitive(name) => assert_eq!(name, ext_type),
                other => panic!("Expected Primitive({}), got {:?}", ext_type, other),
            }
        }
    }

    #[test]
    fn test_parse_reference_type() {
        let ty = parse_type_str("&str");
        match parse_type(&ty) {
            RustType::Primitive(name) => assert_eq!(name, "String"),
            other => panic!("Expected Primitive(String) from &str, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_reference_string() {
        let ty = parse_type_str("&String");
        match parse_type(&ty) {
            RustType::Primitive(name) => assert_eq!(name, "String"),
            other => panic!("Expected Primitive(String) from &String, got {:?}", other),
        }
    }
}

