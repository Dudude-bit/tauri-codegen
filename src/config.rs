use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub input: InputConfig,
    pub output: OutputConfig,
    #[serde(default)]
    pub naming: NamingConfig,
}

/// Input configuration - where to find Rust source files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    /// Directory to scan for Rust files
    pub source_dir: PathBuf,
    /// Directories or files to exclude from scanning
    #[serde(default)]
    pub exclude: Vec<String>,
}

/// Output configuration - where to write generated TypeScript files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Path for generated TypeScript types file
    pub types_file: PathBuf,
    /// Path for generated TypeScript commands file
    pub commands_file: PathBuf,
}

/// Naming configuration - prefixes and suffixes for generated code
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NamingConfig {
    /// Prefix for TypeScript type names
    #[serde(default)]
    pub type_prefix: String,
    /// Suffix for TypeScript type names
    #[serde(default)]
    pub type_suffix: String,
    /// Prefix for TypeScript function names
    #[serde(default)]
    pub function_prefix: String,
    /// Suffix for TypeScript function names
    #[serde(default)]
    pub function_suffix: String,
}

impl Config {
    /// Load configuration from a TOML file
    pub fn load(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        config.validate()?;

        Ok(config)
    }

    /// Validate the configuration
    fn validate(&self) -> Result<()> {
        if !self.input.source_dir.exists() {
            anyhow::bail!(
                "Source directory does not exist: {}",
                self.input.source_dir.display()
            );
        }

        // Ensure output directories exist or can be created
        if let Some(parent) = self.output.types_file.parent() {
            if !parent.exists() && !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create output directory: {}", parent.display())
                })?;
            }
        }

        if let Some(parent) = self.output.commands_file.parent() {
            if !parent.exists() && !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create output directory: {}", parent.display())
                })?;
            }
        }

        Ok(())
    }

    /// Generate a default configuration
    pub fn default_config() -> Self {
        Config {
            input: InputConfig {
                source_dir: PathBuf::from("src-tauri/src"),
                exclude: vec!["tests".to_string(), "target".to_string()],
            },
            output: OutputConfig {
                types_file: PathBuf::from("src/generated/types.ts"),
                commands_file: PathBuf::from("src/generated/commands.ts"),
            },
            naming: NamingConfig::default(),
        }
    }

    /// Save configuration to a TOML file
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize configuration")?;

        fs::write(path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }
}

