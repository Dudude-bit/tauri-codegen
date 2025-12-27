use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// CLI tool to generate TypeScript bindings from Tauri commands
#[derive(Parser, Debug)]
#[command(name = "tauri-ts-generator")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Generate TypeScript bindings from Rust Tauri commands
    Generate {
        /// Path to the configuration file
        #[arg(short, long, default_value = "tauri-codegen.toml")]
        config: PathBuf,

        /// Enable verbose output
        #[arg(short, long, default_value = "false")]
        verbose: bool,
    },

    /// Initialize a new configuration file
    Init {
        /// Path where to create the configuration file
        #[arg(short, long, default_value = "tauri-codegen.toml")]
        output: PathBuf,

        /// Overwrite existing configuration file
        #[arg(short, long, default_value = "false")]
        force: bool,
    },
}

impl Cli {
    pub fn parse_args() -> Self {
        Cli::parse()
    }
}

