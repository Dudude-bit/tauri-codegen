//! # tauri-codegen
//!
//! A CLI tool and library for generating TypeScript bindings from Tauri commands.
//!
//! This crate scans your Rust code for `#[tauri::command]` macros and automatically generates:
//! - **TypeScript interfaces** for your Rust structs and enums.
//! - **TypeScript wrapper functions** to invoke your commands.
//!
//! It is designed to ensure type safety between your Rust backend and TypeScript frontend,
//! reducing boilerplate and runtime errors.
//!
//! ## Features
//!
//! - **Automated Parsing**: Uses `syn` to parse Rust AST.
//! - **Async Support**: Correctly handles `async` commands.
//! - **Type Mapping**: Converts Rust types to their TypeScript equivalents.
//! - **Custom Types**: Supports `struct` and `enum` with `serde` serialization.
//!
//! ## Usage
//!
//! Although primarily used as a CLI tool, you can also use it as a library:
//!
//! ```rust,no_run
//! use tauri_codegen::config::Config;
//! use tauri_codegen::pipeline::Pipeline;
//!
//! fn main() -> anyhow::Result<()> {
//!     let config = Config::default_config();
//!     let pipeline = Pipeline::new(config);
//!     pipeline.run()?;
//!     Ok(())
//! }
//! ```

pub mod cli;
pub mod config;
pub mod generator;
pub mod models;
pub mod parser;
pub mod pipeline;
pub mod resolver;
pub mod scanner;
pub mod utils;

