use crate::known_types;
use crate::models::RustType;

use super::GeneratorContext;

// Re-export to_camel_case for tests
#[cfg(test)]
use crate::utils::to_camel_case;

/// Convert a Rust type to its TypeScript equivalent
pub fn rust_to_typescript(rust_type: &RustType, ctx: &GeneratorContext) -> String {
    match rust_type {
        RustType::Primitive(name) => primitive_to_typescript(name),

        RustType::Vec(inner) => {
            let inner_ts = rust_to_typescript(inner, ctx);
            // Wrap in parentheses if it's a union type (contains |)
            if inner_ts.contains('|') {
                format!("({})[]", inner_ts)
            } else {
                format!("{}[]", inner_ts)
            }
        }

        RustType::Option(inner) => {
            let inner_ts = rust_to_typescript(inner, ctx);
            format!("{} | null", inner_ts)
        }

        RustType::Result(ok) => {
            // For Result types, we return the Ok type
            // The error will be handled by Promise rejection
            rust_to_typescript(ok, ctx)
        }

        RustType::HashMap { key, value } => {
            let key_ts = rust_to_typescript(key, ctx);
            let value_ts = rust_to_typescript(value, ctx);
            
            // Check if strict key usage is safe for TypeScript Record
            let use_param_key = match &**key {
                // bool keys become strings in JSON ("true"/"false") but are invalid in TS Record<bool, ...>
                RustType::Primitive(p) if p == "bool" => false, 
                // numbers/strings are fine
                RustType::Primitive(_) => true,
                // Custom types (enums, newtypes) are assumed to be valid string/number keys
                RustType::Custom(_) => true,
                // Generic Params are assumed to be valid
                RustType::Generic(_) => true,
                // Complex types (Vec, Option, etc) cannot be keys in TS Record
                _ => false,
            };

            if use_param_key {
                format!("Record<{}, {}>", key_ts, value_ts)
            } else {
                format!("Record<string, {}>", value_ts)
            }
        }

        RustType::Tuple(types) => {
            if types.is_empty() {
                "void".to_string()
            } else {
                let type_strs: Vec<_> = types.iter().map(|t| rust_to_typescript(t, ctx)).collect();
                format!("[{}]", type_strs.join(", "))
            }
        }

        RustType::Custom(name) => {
            if ctx.is_custom_type(name) {
                ctx.format_type_name(name)
            } else {
                // Unknown custom type - use the name as-is
                name.clone()
            }
        }

        RustType::Generic(name) => {
            // Generic type parameters are passed through as-is (T, U, etc.)
            name.clone()
        }

        RustType::Unit => "void".to_string(),

        RustType::Unknown(desc) => {
            eprintln!("Warning: Unknown type '{}', using 'unknown'", desc);
            "unknown".to_string()
        }
    }
}

/// Convert a Rust primitive type name to TypeScript
fn primitive_to_typescript(name: &str) -> String {
    // Use the centralized known_types module
    if let Some(ts_type) = known_types::primitive_to_typescript(name) {
        return ts_type.to_string();
    }
    
    eprintln!(
        "Warning: Unknown primitive type '{}', using 'unknown'",
        name
    );
    "unknown".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NamingConfig;

    fn default_ctx() -> GeneratorContext {
        GeneratorContext::new(NamingConfig::default())
    }

    fn ctx_with_type(type_name: &str) -> GeneratorContext {
        let mut ctx = default_ctx();
        ctx.register_type(type_name);
        ctx
    }

    #[test]
    fn test_primitive_to_typescript() {
        assert_eq!(primitive_to_typescript("String"), "string");
        assert_eq!(primitive_to_typescript("i32"), "number");
        assert_eq!(primitive_to_typescript("u64"), "number");
        assert_eq!(primitive_to_typescript("f32"), "number");
        assert_eq!(primitive_to_typescript("bool"), "boolean");
    }

    #[test]
    fn test_primitive_all_integers() {
        for int_type in ["i8", "i16", "i32", "i64", "i128", "isize"] {
            assert_eq!(primitive_to_typescript(int_type), "number");
        }
        for uint_type in ["u8", "u16", "u32", "u64", "u128", "usize"] {
            assert_eq!(primitive_to_typescript(uint_type), "number");
        }
    }

    #[test]
    fn test_primitive_floats() {
        assert_eq!(primitive_to_typescript("f32"), "number");
        assert_eq!(primitive_to_typescript("f64"), "number");
    }

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("get_user"), "getUser");
        assert_eq!(to_camel_case("get_user_by_id"), "getUserById");
        assert_eq!(to_camel_case("hello"), "hello");
        assert_eq!(to_camel_case("HELLO"), "hELLO");
    }

    #[test]
    fn test_vec_to_typescript() {
        let ctx = default_ctx();
        let ty = RustType::Vec(Box::new(RustType::Primitive("String".to_string())));
        assert_eq!(rust_to_typescript(&ty, &ctx), "string[]");
    }

    #[test]
    fn test_vec_nested() {
        let ctx = default_ctx();
        let ty = RustType::Vec(Box::new(RustType::Vec(Box::new(RustType::Primitive(
            "i32".to_string(),
        )))));
        assert_eq!(rust_to_typescript(&ty, &ctx), "number[][]");
    }

    #[test]
    fn test_option_to_typescript() {
        let ctx = default_ctx();
        let ty = RustType::Option(Box::new(RustType::Primitive("String".to_string())));
        assert_eq!(rust_to_typescript(&ty, &ctx), "string | null");
    }

    #[test]
    fn test_option_custom_type() {
        let ctx = ctx_with_type("User");
        let ty = RustType::Option(Box::new(RustType::Custom("User".to_string())));
        assert_eq!(rust_to_typescript(&ty, &ctx), "User | null");
    }

    #[test]
    fn test_result_to_typescript() {
        let ctx = ctx_with_type("User");
        let ty = RustType::Result(Box::new(RustType::Custom("User".to_string())));
        assert_eq!(rust_to_typescript(&ty, &ctx), "User");
    }

    #[test]
    fn test_result_with_vec() {
        let ctx = ctx_with_type("Item");
        let ty = RustType::Result(Box::new(RustType::Vec(Box::new(RustType::Custom(
            "Item".to_string(),
        )))));
        assert_eq!(rust_to_typescript(&ty, &ctx), "Item[]");
    }

    #[test]
    fn test_hashmap_to_typescript() {
        let ctx = default_ctx();
        let ty = RustType::HashMap {
            key: Box::new(RustType::Primitive("String".to_string())),
            value: Box::new(RustType::Primitive("i32".to_string())),
        };
        assert_eq!(rust_to_typescript(&ty, &ctx), "Record<string, number>");
    }

    #[test]
    fn test_hashmap_with_custom_value() {
        let ctx = ctx_with_type("User");
        let ty = RustType::HashMap {
            key: Box::new(RustType::Primitive("String".to_string())),
            value: Box::new(RustType::Custom("User".to_string())),
        };
        assert_eq!(rust_to_typescript(&ty, &ctx), "Record<string, User>");
    }

    #[test]
    fn test_tuple_to_typescript() {
        let ctx = default_ctx();
        let ty = RustType::Tuple(vec![
            RustType::Primitive("i32".to_string()),
            RustType::Primitive("String".to_string()),
            RustType::Primitive("bool".to_string()),
        ]);
        assert_eq!(rust_to_typescript(&ty, &ctx), "[number, string, boolean]");
    }

    #[test]
    fn test_empty_tuple_to_void() {
        let ctx = default_ctx();
        let ty = RustType::Tuple(vec![]);
        assert_eq!(rust_to_typescript(&ty, &ctx), "void");
    }

    #[test]
    fn test_unit_to_typescript() {
        let ctx = default_ctx();
        let ty = RustType::Unit;
        assert_eq!(rust_to_typescript(&ty, &ctx), "void");
    }

    #[test]
    fn test_custom_type_registered() {
        let ctx = ctx_with_type("User");
        let ty = RustType::Custom("User".to_string());
        assert_eq!(rust_to_typescript(&ty, &ctx), "User");
    }

    #[test]
    fn test_custom_type_unregistered() {
        let ctx = default_ctx();
        let ty = RustType::Custom("UnknownType".to_string());
        assert_eq!(rust_to_typescript(&ty, &ctx), "UnknownType");
    }

    #[test]
    fn test_generic_passthrough() {
        let ctx = default_ctx();
        let ty = RustType::Generic("T".to_string());
        assert_eq!(rust_to_typescript(&ty, &ctx), "T");
    }

    #[test]
    fn test_external_types_chrono() {
        assert_eq!(primitive_to_typescript("DateTime"), "string");
        assert_eq!(primitive_to_typescript("NaiveDateTime"), "string");
        assert_eq!(primitive_to_typescript("NaiveDate"), "string");
        assert_eq!(primitive_to_typescript("NaiveTime"), "string");
    }

    #[test]
    fn test_external_types_time() {
        assert_eq!(primitive_to_typescript("OffsetDateTime"), "string");
        assert_eq!(primitive_to_typescript("PrimitiveDateTime"), "string");
        assert_eq!(primitive_to_typescript("Date"), "string");
        assert_eq!(primitive_to_typescript("Time"), "string");
    }

    #[test]
    fn test_external_types_uuid() {
        assert_eq!(primitive_to_typescript("Uuid"), "string");
    }

    #[test]
    fn test_external_types_decimal() {
        assert_eq!(primitive_to_typescript("Decimal"), "string");
        assert_eq!(primitive_to_typescript("BigDecimal"), "string");
    }

    #[test]
    fn test_external_types_path() {
        assert_eq!(primitive_to_typescript("PathBuf"), "string");
        assert_eq!(primitive_to_typescript("Path"), "string");
    }

    #[test]
    fn test_external_types_network() {
        assert_eq!(primitive_to_typescript("IpAddr"), "string");
        assert_eq!(primitive_to_typescript("Ipv4Addr"), "string");
        assert_eq!(primitive_to_typescript("Ipv6Addr"), "string");
        assert_eq!(primitive_to_typescript("Url"), "string");
    }

    #[test]
    fn test_duration_to_number() {
        assert_eq!(primitive_to_typescript("Duration"), "number");
    }

    #[test]
    fn test_serde_value_to_unknown() {
        assert_eq!(primitive_to_typescript("Value"), "unknown");
    }

    #[test]
    fn test_bytes_to_number_array() {
        assert_eq!(primitive_to_typescript("Bytes"), "number[]");
    }

    #[test]
    fn test_naming_with_prefix() {
        let mut ctx = GeneratorContext::new(NamingConfig {
            type_prefix: "I".to_string(),
            type_suffix: "".to_string(),
            function_prefix: "".to_string(),
            function_suffix: "".to_string(),
        });
        ctx.register_type("User");
        let ty = RustType::Custom("User".to_string());
        assert_eq!(rust_to_typescript(&ty, &ctx), "IUser");
    }

    #[test]
    fn test_naming_with_suffix() {
        let mut ctx = GeneratorContext::new(NamingConfig {
            type_prefix: "".to_string(),
            type_suffix: "DTO".to_string(),
            function_prefix: "".to_string(),
            function_suffix: "".to_string(),
        });
        ctx.register_type("User");
        let ty = RustType::Custom("User".to_string());
        assert_eq!(rust_to_typescript(&ty, &ctx), "UserDTO");
    }

    #[test]
    fn test_complex_nested_type() {
        let ctx = ctx_with_type("User");
        let ty = RustType::Result(Box::new(RustType::Vec(Box::new(RustType::Option(
            Box::new(RustType::Custom("User".to_string())),
        )))));
        assert_eq!(rust_to_typescript(&ty, &ctx), "(User | null)[]");
    }
}
