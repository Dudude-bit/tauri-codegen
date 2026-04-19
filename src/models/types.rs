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
    /// Serialization shape — how serde wires this struct onto the JSON output.
    /// Derived from the AST form and `#[serde(transparent)]`.
    pub shape: StructShape,
    /// Source file where the struct was found
    pub source_file: PathBuf,
}

/// How serde serializes this struct.
///
/// - `Named` — `struct Foo { a: T }` → `{ "a": T }`.
/// - `Newtype` — `struct Foo(T)` or a `#[serde(transparent)]` one-field
///   struct → `T` (no wrapper).
/// - `Tuple` — `struct Foo(T1, T2)` → `[T1, T2]`.
/// - `Unit` — `struct Foo;` → `null`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StructShape {
    #[default]
    Named,
    Newtype,
    Tuple,
    Unit,
}

/// Represents a struct field
#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    /// Field name as it appears in serialized output.
    /// Already reflects serde `rename`/`rename_all` transformations applied by the parser.
    pub name: String,
    /// Field type
    pub ty: RustType,
    /// Whether the name came from a serde `rename` or `rename_all` attribute.
    /// Informational only — `name` is always authoritative for TypeScript output.
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
    /// Variant name as it appears in serialized output.
    /// Already reflects serde `rename`/`rename_all` transformations applied by the parser.
    pub name: String,
    /// Variant data (for tuple/struct variants)
    pub data: VariantData,
    /// Whether the name came from a serde `rename` or `rename_all` attribute.
    /// Informational only — `name` is always authoritative for TypeScript output.
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
