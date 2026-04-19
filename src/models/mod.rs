//! Data models for representing Tauri commands and Rust types.

mod command;
mod rust_type;
mod types;

pub use command::{CommandArg, TauriCommand};
pub use rust_type::{walk_custom_type_names, RustType};
pub use types::{
    EnumRepresentation, EnumVariant, RustEnum, RustStruct, RustTypeAlias, StructField, StructShape,
    VariantData,
};
