pub mod commands_gen;
pub mod type_mapper;
pub mod types_gen;

use crate::config::NamingConfig;

/// Context for code generation
pub struct GeneratorContext {
    /// Naming configuration
    pub naming: NamingConfig,
    /// Set of custom type names that are available
    pub custom_types: std::collections::HashSet<String>,
}

impl GeneratorContext {
    pub fn new(naming: NamingConfig) -> Self {
        Self {
            naming,
            custom_types: std::collections::HashSet::new(),
        }
    }

    /// Add a custom type name to the context
    pub fn register_type(&mut self, name: &str) {
        self.custom_types.insert(name.to_string());
    }

    /// Check if a type name is registered as a custom type
    pub fn is_custom_type(&self, name: &str) -> bool {
        self.custom_types.contains(name)
    }

    /// Apply naming configuration to a type name
    pub fn format_type_name(&self, name: &str) -> String {
        format!("{}{}{}", self.naming.type_prefix, name, self.naming.type_suffix)
    }

    /// Apply naming configuration to a function name
    pub fn format_function_name(&self, name: &str) -> String {
        format!(
            "{}{}{}",
            self.naming.function_prefix, name, self.naming.function_suffix
        )
    }
}

