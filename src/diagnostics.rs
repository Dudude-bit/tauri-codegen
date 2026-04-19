//! Diagnostics sink.
//!
//! Two classes of user-facing message coexist in this crate:
//!
//! 1. **Pipeline status** — "Generated: X", "Parsed N commands",
//!    cargo-expand progress, per-file debug detail. Driven by `--verbose`
//!    and routed exclusively through this `Diagnostics` type.
//!
//! 2. **Always-on warnings from parsers and generators** — "Unknown
//!    rename_all convention", "#[ts(optional)] on non-Option field",
//!    "Unknown primitive type". These indicate real problems the user
//!    must see regardless of verbosity, so they stay as direct
//!    `eprintln!` with a uniform `"Warning: "` prefix. Threading
//!    `Diagnostics` through the pure parsing/generation helpers would
//!    add signatures-as-plumbing for no observable benefit.
//!
//! Keeping the door open for structured output (JSON, tracing) later is
//! still easy: those would replace *both* sinks at once, so the current
//! split doesn't lock us in.

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
