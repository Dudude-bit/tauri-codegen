//! Diagnostics sink: the pipeline routes all user-facing messages through
//! this type instead of calling `println!`/`eprintln!` directly. Centralising
//! the output makes it trivial to respect `--verbose` consistently and keeps
//! the door open for structured output (JSON, logger) later.

use std::fmt::Display;

/// Reports progress, warnings, and verbose-only detail to the terminal.
#[derive(Debug, Clone, Copy)]
pub struct Diagnostics {
    verbose: bool,
}

impl Diagnostics {
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }

    pub fn verbose(&self) -> bool {
        self.verbose
    }

    /// Status lines that users expect to see on every run
    /// ("Generated: …", "Parsed N commands", "Done!").
    pub fn info(&self, msg: impl Display) {
        println!("{}", msg);
    }

    /// Non-fatal warnings that should always be visible.
    pub fn warn(&self, msg: impl Display) {
        eprintln!("Warning: {}", msg);
    }

    /// Hard errors emitted before the pipeline bails; visible by default.
    pub fn error(&self, msg: impl Display) {
        eprintln!("Error: {}", msg);
    }

    /// Internal detail useful for debugging scanning/parsing.
    /// Suppressed unless `--verbose` was passed.
    pub fn debug(&self, msg: impl Display) {
        if self.verbose {
            eprintln!("Info: {}", msg);
        }
    }
}
