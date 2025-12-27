/// Represents a Rust type
#[derive(Debug, Clone)]
pub enum RustType {
    /// Primitive types (String, i32, bool, etc.)
    Primitive(String),
    /// `Vec<T>`
    Vec(Box<RustType>),
    /// `Option<T>`
    Option(Box<RustType>),
    /// `Result<T, E>` - only Ok type is used for TypeScript generation
    Result(Box<RustType>),
    /// `HashMap<K, V>`
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

