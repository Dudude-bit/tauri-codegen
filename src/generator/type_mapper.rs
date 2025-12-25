use crate::parser::RustType;

use super::GeneratorContext;

/// Convert a Rust type to its TypeScript equivalent
pub fn rust_to_typescript(rust_type: &RustType, ctx: &GeneratorContext) -> String {
    match rust_type {
        RustType::Primitive(name) => primitive_to_typescript(name),

        RustType::Vec(inner) => {
            let inner_ts = rust_to_typescript(inner, ctx);
            format!("{}[]", inner_ts)
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
            format!("Record<{}, {}>", key_ts, value_ts)
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
    match name {
        "String" | "str" | "char" => "string".to_string(),

        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128"
        | "usize" | "f32" | "f64" => "number".to_string(),

        "bool" => "boolean".to_string(),

        // chrono types - serialize to ISO 8601 strings
        "DateTime" | "NaiveDateTime" | "NaiveDate" | "NaiveTime" => "string".to_string(),
        
        // time crate types
        "OffsetDateTime" | "PrimitiveDateTime" | "Date" | "Time" => "string".to_string(),
        
        // UUID
        "Uuid" => "string".to_string(),
        
        // Decimal types
        "Decimal" | "BigDecimal" => "string".to_string(),
        
        // Path types
        "PathBuf" | "Path" => "string".to_string(),
        
        // Network types
        "IpAddr" | "Ipv4Addr" | "Ipv6Addr" | "Url" => "string".to_string(),
        
        // Duration (typically serialized as number - seconds or milliseconds)
        "Duration" => "number".to_string(),
        
        // serde_json::Value - any JSON value
        "Value" => "unknown".to_string(),
        
        // Bytes
        "Bytes" => "number[]".to_string(),

        _ => {
            eprintln!(
                "Warning: Unknown primitive type '{}', using 'unknown'",
                name
            );
            "unknown".to_string()
        }
    }
}

/// Convert snake_case to camelCase
pub fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;

    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else if i == 0 {
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_to_typescript() {
        assert_eq!(primitive_to_typescript("String"), "string");
        assert_eq!(primitive_to_typescript("i32"), "number");
        assert_eq!(primitive_to_typescript("u64"), "number");
        assert_eq!(primitive_to_typescript("f32"), "number");
        assert_eq!(primitive_to_typescript("bool"), "boolean");
    }

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("get_user"), "getUser");
        assert_eq!(to_camel_case("get_user_by_id"), "getUserById");
        assert_eq!(to_camel_case("hello"), "hello");
        assert_eq!(to_camel_case("HELLO"), "hELLO");
    }
}

