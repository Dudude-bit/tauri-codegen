//! Known types module - centralized list of known Rust types and their TypeScript mappings
//!
//! This module provides a single source of truth for type recognition,
//! eliminating duplication between type_extractor.rs and type_mapper.rs

/// Primitive/built-in types that map to simple TypeScript types
pub const PRIMITIVE_STRING_TYPES: &[&str] = &["String", "str", "char"];

/// Signed integer types
pub const SIGNED_INTEGER_TYPES: &[&str] = &["i8", "i16", "i32", "i64", "i128", "isize"];

/// Unsigned integer types  
pub const UNSIGNED_INTEGER_TYPES: &[&str] = &["u8", "u16", "u32", "u64", "u128", "usize"];

/// Floating point types
pub const FLOAT_TYPES: &[&str] = &["f32", "f64"];

/// Boolean type
pub const BOOL_TYPE: &str = "bool";

/// External types that serialize to strings (chrono, time, uuid, etc.)
pub const EXTERNAL_STRING_TYPES: &[&str] = &[
    // chrono
    "DateTime",
    "NaiveDateTime",
    "NaiveDate",
    "NaiveTime",
    // time crate
    "OffsetDateTime",
    "PrimitiveDateTime",
    "Date",
    "Time",
    // uuid
    "Uuid",
    // decimal
    "Decimal",
    "BigDecimal",
    // std::path
    "PathBuf",
    "Path",
    // network types
    "IpAddr",
    "Ipv4Addr",
    "Ipv6Addr",
    // url
    "Url",
];

/// Types that serialize to numbers
pub const EXTERNAL_NUMBER_TYPES: &[&str] = &["Duration"];

/// serde_json::Value - any JSON value
pub const JSON_VALUE_TYPE: &str = "Value";

/// Bytes type
pub const BYTES_TYPE: &str = "Bytes";

/// Check if a type name is a known primitive type
pub fn is_primitive_type(name: &str) -> bool {
    PRIMITIVE_STRING_TYPES.contains(&name)
        || SIGNED_INTEGER_TYPES.contains(&name)
        || UNSIGNED_INTEGER_TYPES.contains(&name)
        || FLOAT_TYPES.contains(&name)
        || name == BOOL_TYPE
}

/// Check if a type name is a known external type (serializes to string)
pub fn is_external_string_type(name: &str) -> bool {
    EXTERNAL_STRING_TYPES.contains(&name)
}

/// Check if a type name is a known external type (serializes to number)
pub fn is_external_number_type(name: &str) -> bool {
    EXTERNAL_NUMBER_TYPES.contains(&name)
}

/// Get the TypeScript type for a primitive Rust type name
pub fn primitive_to_typescript(name: &str) -> Option<&'static str> {
    if PRIMITIVE_STRING_TYPES.contains(&name) {
        return Some("string");
    }
    if SIGNED_INTEGER_TYPES.contains(&name)
        || UNSIGNED_INTEGER_TYPES.contains(&name)
        || FLOAT_TYPES.contains(&name)
    {
        return Some("number");
    }
    if name == BOOL_TYPE {
        return Some("boolean");
    }
    if EXTERNAL_STRING_TYPES.contains(&name) {
        return Some("string");
    }
    if EXTERNAL_NUMBER_TYPES.contains(&name) {
        return Some("number");
    }
    if name == JSON_VALUE_TYPE {
        return Some("unknown");
    }
    if name == BYTES_TYPE {
        return Some("number[]");
    }
    None
}

/// Tauri special types that are injected by the framework and not passed from frontend
pub const TAURI_SPECIAL_TYPES: &[&str] = &["State", "Window", "AppHandle", "Webview", "WebviewWindow"];

/// Check if a type name is a Tauri special type (injected by framework)
pub fn is_tauri_special_type(name: &str) -> bool {
    TAURI_SPECIAL_TYPES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_primitive_type() {
        assert!(is_primitive_type("String"));
        assert!(is_primitive_type("i32"));
        assert!(is_primitive_type("u64"));
        assert!(is_primitive_type("f64"));
        assert!(is_primitive_type("bool"));
        assert!(!is_primitive_type("User"));
        assert!(!is_primitive_type("DateTime"));
    }

    #[test]
    fn test_is_external_string_type() {
        assert!(is_external_string_type("DateTime"));
        assert!(is_external_string_type("Uuid"));
        assert!(is_external_string_type("PathBuf"));
        assert!(!is_external_string_type("String"));
        assert!(!is_external_string_type("User"));
    }

    #[test]
    fn test_primitive_to_typescript() {
        assert_eq!(primitive_to_typescript("String"), Some("string"));
        assert_eq!(primitive_to_typescript("str"), Some("string"));
        assert_eq!(primitive_to_typescript("i32"), Some("number"));
        assert_eq!(primitive_to_typescript("u64"), Some("number"));
        assert_eq!(primitive_to_typescript("f64"), Some("number"));
        assert_eq!(primitive_to_typescript("bool"), Some("boolean"));
        assert_eq!(primitive_to_typescript("DateTime"), Some("string"));
        assert_eq!(primitive_to_typescript("Duration"), Some("number"));
        assert_eq!(primitive_to_typescript("Value"), Some("unknown"));
        assert_eq!(primitive_to_typescript("Bytes"), Some("number[]"));
        assert_eq!(primitive_to_typescript("User"), None);
    }
}
