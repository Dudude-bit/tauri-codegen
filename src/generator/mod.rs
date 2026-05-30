pub mod commands_gen;
pub mod helpers_gen;
pub mod type_mapper;
pub mod types_gen;

use std::collections::HashSet;

use crate::config::NamingConfig;

/// Context for code generation.
///
/// Fields are private so every mutation goes through `register_type` (the
/// custom-type registry must stay consistent with the naming config).
/// Consumers read via the `format_*` / `is_custom_type` accessors.
pub struct GeneratorContext {
    naming: NamingConfig,
    custom_types: HashSet<String>,
    enum_types: HashSet<String>,
}

impl GeneratorContext {
    pub fn new(naming: NamingConfig) -> Self {
        Self {
            naming,
            custom_types: HashSet::new(),
            enum_types: HashSet::new(),
        }
    }

    /// Add a custom struct / alias type name to the context.
    pub fn register_type(&mut self, name: &str) {
        self.custom_types.insert(name.to_string());
    }

    /// Add a custom enum type name. Automatically registers it as a custom
    /// type as well so the regular membership check continues to work.
    pub fn register_enum(&mut self, name: &str) {
        self.custom_types.insert(name.to_string());
        self.enum_types.insert(name.to_string());
    }

    /// Check if a type name is registered as a custom type.
    pub fn is_custom_type(&self, name: &str) -> bool {
        self.custom_types.contains(name)
    }

    /// Check if a type name is registered as an *enum* (as opposed to a
    /// struct or alias). Used to warn on misuse, e.g. `#[serde(flatten)]`
    /// targeting an enum, where the resulting TypeScript intersection with
    /// a string-union reduces to `never`.
    pub fn is_enum(&self, name: &str) -> bool {
        self.enum_types.contains(name)
    }

    /// Apply naming configuration to a type name.
    pub fn format_type_name(&self, name: &str) -> String {
        format!(
            "{}{}{}",
            self.naming.type_prefix, name, self.naming.type_suffix
        )
    }

    /// Apply naming configuration to a function name.
    pub fn format_function_name(&self, name: &str) -> String {
        format!(
            "{}{}{}",
            self.naming.function_prefix, name, self.naming.function_suffix
        )
    }
}
