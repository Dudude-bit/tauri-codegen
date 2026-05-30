//! End-to-end tests for tauri-ts-generator.
//!
//! Each test spins up a fresh temp directory, writes a minimal Tauri-like
//! project (Rust source + `tauri-codegen.toml`), runs the compiled CLI
//! binary against it, and asserts both the exit status and the content of
//! the generated TypeScript files. Unlike the unit/integration tests, these
//! exercise the full path: argument parsing → config loading → scan → parse
//! → resolve → generate → file write.
//!
//! To refresh expected output after an intentional change, run with
//! `UPDATE_GOLDEN=1 cargo test --test e2e` and commit the diff.

mod helpers;

mod basic;
mod channels;
mod errors;
mod init;
mod path_qualified;
mod recursive;
mod rename_all;
mod serde_features;
mod smart_pointers;
mod snapshots;
mod struct_shapes;
