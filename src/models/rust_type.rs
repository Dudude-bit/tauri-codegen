/// Represents a Rust type
#[derive(Debug, Clone, PartialEq)]
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

/// Walk a `RustType` tree, invoking `visit` for every `Custom(name)` node.
///
/// Both the command collector (what types are reachable?) and the
/// import collector (what types are referenced in a generated file?)
/// need the same recursion; only the visitor action differs.
pub fn walk_custom_type_names<F: FnMut(&str)>(ty: &RustType, visit: &mut F) {
    match ty {
        RustType::Custom(name) => visit(name),
        RustType::Vec(inner) | RustType::Option(inner) | RustType::Result(inner) => {
            walk_custom_type_names(inner, visit);
        }
        RustType::HashMap { key, value } => {
            walk_custom_type_names(key, visit);
            walk_custom_type_names(value, visit);
        }
        RustType::Tuple(types) => {
            for t in types {
                walk_custom_type_names(t, visit);
            }
        }
        RustType::Primitive(_) | RustType::Generic(_) | RustType::Unit | RustType::Unknown(_) => {}
    }
}
