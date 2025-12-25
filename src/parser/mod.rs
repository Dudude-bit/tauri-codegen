pub mod command;
pub mod types;

/// Represents a parsed Tauri command
#[derive(Debug, Clone)]
pub struct TauriCommand {
    /// Name of the command (function name)
    pub name: String,
    /// Function arguments
    pub args: Vec<CommandArg>,
    /// Return type (None for functions returning ())
    pub return_type: Option<RustType>,
}

/// Represents a function argument
#[derive(Debug, Clone)]
pub struct CommandArg {
    /// Argument name
    pub name: String,
    /// Argument type
    pub ty: RustType,
}

/// Represents a parsed Rust struct
#[derive(Debug, Clone)]
pub struct RustStruct {
    /// Name of the struct
    pub name: String,
    /// Generic type parameters (e.g., ["T", "U"])
    pub generics: Vec<String>,
    /// Struct fields
    pub fields: Vec<StructField>,
    /// Source file where the struct was found
    pub source_file: std::path::PathBuf,
}

/// Represents a struct field
#[derive(Debug, Clone)]
pub struct StructField {
    /// Field name
    pub name: String,
    /// Field type
    pub ty: RustType,
}

/// Represents a parsed Rust enum
#[derive(Debug, Clone)]
pub struct RustEnum {
    /// Name of the enum
    pub name: String,
    /// Enum variants
    pub variants: Vec<EnumVariant>,
    /// Source file where the enum was found
    pub source_file: std::path::PathBuf,
}

/// Represents an enum variant
#[derive(Debug, Clone)]
pub struct EnumVariant {
    /// Variant name
    pub name: String,
    /// Variant data (for tuple/struct variants)
    pub data: VariantData,
}

/// Represents the data associated with an enum variant
#[derive(Debug, Clone)]
pub enum VariantData {
    /// Unit variant (no data)
    Unit,
    /// Tuple variant with types
    Tuple(Vec<RustType>),
    /// Struct variant with named fields
    Struct(Vec<StructField>),
}

/// Represents a Rust type
#[derive(Debug, Clone)]
pub enum RustType {
    /// Primitive types (String, i32, bool, etc.)
    Primitive(String),
    /// Vec<T>
    Vec(Box<RustType>),
    /// Option<T>
    Option(Box<RustType>),
    /// Result<T, E> - only Ok type is used for TypeScript generation
    Result(Box<RustType>),
    /// HashMap<K, V>
    HashMap {
        key: Box<RustType>,
        value: Box<RustType>,
    },
    /// Tuple types
    Tuple(Vec<RustType>),
    /// Reference to a custom type (struct or enum)
    Custom(String),
    /// Generic type parameter (T, U, K, V, etc.)
    Generic(String),
    /// Unit type ()
    Unit,
    /// Unknown type (fallback)
    Unknown(String),
}

/// Result of parsing a Rust file
#[derive(Debug, Default)]
pub struct ParseResult {
    /// Tauri commands found in the file
    pub commands: Vec<TauriCommand>,
    /// Structs found in the file
    pub structs: Vec<RustStruct>,
    /// Enums found in the file
    pub enums: Vec<RustEnum>,
}

impl ParseResult {
    pub fn new() -> Self {
        Self::default()
    }
}
