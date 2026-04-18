use super::{RustEnum, RustStruct, TauriCommand};

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
