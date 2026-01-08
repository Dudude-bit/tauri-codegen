use std::path::PathBuf;

use super::RustType;

/// Represents a parsed Rust struct
#[derive(Debug, Clone, PartialEq)]
pub struct RustStruct {
    /// Name of the struct
    pub name: String,
    /// Generic type parameters (e.g., ["T", "U"])
    pub generics: Vec<String>,
    /// Struct fields
    pub fields: Vec<StructField>,
    /// Source file where the struct was found
    pub source_file: PathBuf,
}

/// Represents a struct field
#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    /// Field name (may be renamed via serde)
    pub name: String,
    /// Field type
    pub ty: RustType,
    /// Whether the name was set via serde rename or rename_all
    /// If true, the name should be used as-is without camelCase conversion
    pub has_explicit_rename: bool,
    /// Whether to use undefined instead of null for Option types
    /// Set via #[ts(optional)] attribute
    pub use_optional: bool,
    /// Whether the field is flattened via #[serde(flatten)]
    /// If true, the field's type will be intersected with the parent type in TypeScript
    pub is_flatten: bool,
}

/// Represents a parsed Rust enum
#[derive(Debug, Clone, PartialEq)]
pub struct RustEnum {
    /// Name of the enum
    pub name: String,
    /// Generic type parameters (e.g., ["T", "U"])
    pub generics: Vec<String>,
    /// Enum variants
    pub variants: Vec<EnumVariant>,
    /// Source file where the enum was found
    pub source_file: PathBuf,
    /// Serde representation of the enum (External, Internal, Adjacent, Untagged)
    pub representation: EnumRepresentation,
}

/// Represents a parsed Rust type alias
#[derive(Debug, Clone, PartialEq)]
pub struct RustTypeAlias {
    /// Name of the alias
    pub name: String,
    /// Generic type parameters (e.g., ["T", "U"])
    pub generics: Vec<String>,
    /// Alias target type
    pub target: RustType,
    /// Source file where the alias was found
    pub source_file: PathBuf,
}

/// Represents the serde representation of an enum
#[derive(Debug, Clone, PartialEq, Default)]
pub enum EnumRepresentation {
    /// default: { "Variant": { ... } }
    #[default]
    External,
    /// #[serde(tag = "type")] -> { "type": "Variant", ... }
    Internal { tag: String },
    /// #[serde(tag = "t", content = "c")] -> { "t": "Variant", "c": { ... } }
    Adjacent { tag: String, content: String },
    /// #[serde(untagged)] -> { ... }
    Untagged,
}

/// Represents an enum variant
#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    /// Variant name (may be renamed via serde)
    pub name: String,
    /// Variant data (for tuple/struct variants)
    pub data: VariantData,
    /// Whether the name was set via serde rename or rename_all
    /// If true, the name should be used as-is without transformation
    pub has_explicit_rename: bool,
}

/// Represents the data associated with an enum variant
#[derive(Debug, Clone, PartialEq)]
pub enum VariantData {
    /// Unit variant (no data)
    Unit,
    /// Tuple variant with types
    Tuple(Vec<RustType>),
    /// Struct variant with named fields
    Struct(Vec<StructField>),
}
