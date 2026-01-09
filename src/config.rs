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
    /// Use cargo expand to handle macro-generated types (slower but more complete)
    #[serde(default)]
    pub use_cargo_expand: bool,
    /// Cargo manifest path for cargo expand (defaults to Cargo.toml in source_dir parent)
    #[serde(default)]
    pub cargo_manifest: Option<PathBuf>,
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

    /// Validate the configuration (pure validation, no side effects)
    fn validate(&self) -> Result<()> {
        if !self.input.source_dir.exists() {
            anyhow::bail!(
                "Source directory does not exist: {}",
                self.input.source_dir.display()
            );
        }
        Ok(())
    }

    /// Ensure output directories exist, creating them if necessary
    /// This should be called before writing output files.
    pub fn ensure_output_directories(&self) -> Result<()> {
        // Note: create_dir_all is idempotent and handles race conditions
        if let Some(parent) = self.output.types_file.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create output directory: {}", parent.display())
                })?;
            }
        }

        if let Some(parent) = self.output.commands_file.parent() {
            if !parent.as_os_str().is_empty() {
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
                use_cargo_expand: false,
                cargo_manifest: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = Config::default_config();

        assert_eq!(config.input.source_dir, PathBuf::from("src-tauri/src"));
        assert!(config.input.exclude.contains(&"tests".to_string()));
        assert!(config.input.exclude.contains(&"target".to_string()));
        assert_eq!(
            config.output.types_file,
            PathBuf::from("src/generated/types.ts")
        );
        assert_eq!(
            config.output.commands_file,
            PathBuf::from("src/generated/commands.ts")
        );
        assert!(config.naming.type_prefix.is_empty());
        assert!(config.naming.type_suffix.is_empty());
    }

    #[test]
    fn test_load_valid_config() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("src");
        fs::create_dir_all(&source_dir).unwrap();

        let config_content = format!(
            r#"
[input]
source_dir = "{}"
exclude = ["tests"]

[output]
types_file = "types.ts"
commands_file = "commands.ts"
"#,
            source_dir.display()
        );

        let config_path = dir.path().join("config.toml");
        let mut file = fs::File::create(&config_path).unwrap();
        file.write_all(config_content.as_bytes()).unwrap();

        let config = Config::load(&config_path).unwrap();

        assert_eq!(config.input.source_dir, source_dir);
        assert_eq!(config.input.exclude, vec!["tests"]);
        assert_eq!(config.output.types_file, PathBuf::from("types.ts"));
    }

    #[test]
    fn test_load_config_with_naming() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("src");
        fs::create_dir_all(&source_dir).unwrap();

        let config_content = format!(
            r#"
[input]
source_dir = "{}"

[output]
types_file = "types.ts"
commands_file = "commands.ts"

[naming]
type_prefix = "I"
type_suffix = "DTO"
function_prefix = "api"
function_suffix = "Cmd"
"#,
            source_dir.display()
        );

        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();

        assert_eq!(config.naming.type_prefix, "I");
        assert_eq!(config.naming.type_suffix, "DTO");
        assert_eq!(config.naming.function_prefix, "api");
        assert_eq!(config.naming.function_suffix, "Cmd");
    }

    #[test]
    fn test_load_config_missing_naming_uses_defaults() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("src");
        fs::create_dir_all(&source_dir).unwrap();

        let config_content = format!(
            r#"
[input]
source_dir = "{}"

[output]
types_file = "types.ts"
commands_file = "commands.ts"
"#,
            source_dir.display()
        );

        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();

        assert!(config.naming.type_prefix.is_empty());
        assert!(config.naming.type_suffix.is_empty());
    }

    #[test]
    fn test_load_invalid_toml() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, "this is not valid toml [[[").unwrap();

        let result = Config::load(&config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_missing_file() {
        let path = PathBuf::from("/nonexistent/path/config.toml");
        let result = Config::load(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_missing_source_dir() {
        let dir = tempdir().unwrap();

        let config_content = r#"
[input]
source_dir = "/nonexistent/source/dir"

[output]
types_file = "types.ts"
commands_file = "commands.ts"
"#;

        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, config_content).unwrap();

        let result = Config::load(&config_path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Source directory does not exist"));
    }

    #[test]
    fn test_save_config() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("output.toml");

        let config = Config {
            input: InputConfig {
                source_dir: PathBuf::from("src"),
                exclude: vec!["tests".to_string()],
                use_cargo_expand: false,
                cargo_manifest: None,
            },
            output: OutputConfig {
                types_file: PathBuf::from("types.ts"),
                commands_file: PathBuf::from("commands.ts"),
            },
            naming: NamingConfig {
                type_prefix: "I".to_string(),
                type_suffix: "".to_string(),
                function_prefix: "".to_string(),
                function_suffix: "".to_string(),
            },
        };

        config.save(&config_path).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("source_dir = \"src\""));
        assert!(content.contains("types_file = \"types.ts\""));
        assert!(content.contains("type_prefix = \"I\""));
    }

    #[test]
    fn test_naming_config_default() {
        let naming = NamingConfig::default();

        assert!(naming.type_prefix.is_empty());
        assert!(naming.type_suffix.is_empty());
        assert!(naming.function_prefix.is_empty());
        assert!(naming.function_suffix.is_empty());
    }

    #[test]
    fn test_config_with_empty_exclude() {
        let dir = tempdir().unwrap();
        let source_dir = dir.path().join("src");
        fs::create_dir_all(&source_dir).unwrap();

        let config_content = format!(
            r#"
[input]
source_dir = "{}"

[output]
types_file = "types.ts"
commands_file = "commands.ts"
"#,
            source_dir.display()
        );

        let config_path = dir.path().join("config.toml");
        fs::write(&config_path, config_content).unwrap();

        let config = Config::load(&config_path).unwrap();

        assert!(config.input.exclude.is_empty());
    }
}
