//! Diagnostics sink.
//!
//! One type, two consumption patterns:
//!
//! 1. **Pipeline status** — "Generated: X", "Parsed N commands",
//!    cargo-expand progress, per-file debug detail. The `Pipeline`
//!    holds a `Diagnostics` instance and calls `info/warn/error/debug`
//!    on it directly. These messages respect `--verbose`.
//!
//! 2. **Warnings from pure parsing/generation helpers** — "Unknown
//!    rename_all convention", "#[ts(optional)] on non-Option field",
//!    "Unknown primitive type". These are called from deep inside the
//!    parser/generator where threading `&Diagnostics` through every
//!    helper would double the signature noise. The pipeline installs
//!    a thread-local at startup and helpers read it via `current()`.
//!    Single-threaded by design — this CLI never forks work onto
//!    rayon/tokio — so a thread-local `Cell` is both sound and cheap.
//!
//! Future structured output (JSON / tracing) replaces this module wholesale.

use std::cell::Cell;
use std::fmt::Display;

/// Reports progress, warnings, and verbose-only detail to the terminal.
#[derive(Debug, Clone, Copy)]
pub struct Diagnostics {
    verbose: bool,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::new(false)
    }
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

thread_local! {
    /// Ambient diagnostics sink for helpers that can't easily take a
    /// `&Diagnostics` parameter (e.g. the attribute-parsing helpers deep
    /// inside the serde walker). Defaults to a silent-default sink before
    /// `Pipeline::run` installs the real one.
    static CURRENT: Cell<Diagnostics> = const { Cell::new(Diagnostics { verbose: false }) };
}

/// Install the ambient sink for the duration of the current thread. Call
/// this once from the pipeline; helpers reached via `current()` will pick
/// up the value.
pub fn install(diag: Diagnostics) {
    CURRENT.with(|c| c.set(diag));
}

/// Read the ambient sink. Returns the silent default if `install` was
/// never called on this thread (tests, library users).
pub fn current() -> Diagnostics {
    CURRENT.with(|c| c.get())
}

/// Shorthand: always-visible warning via the ambient sink.
pub fn warn(msg: impl Display) {
    current().warn(msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_and_current_roundtrip() {
        // Default before any install.
        let before = current();
        assert!(!before.verbose(), "default sink must be silent");

        install(Diagnostics::new(true));
        assert!(
            current().verbose(),
            "install must take effect on this thread"
        );

        // Overriding replaces; the previous verbose value doesn't sneak back.
        install(Diagnostics::new(false));
        assert!(!current().verbose(), "overrides must replace, not stack");
    }

    #[test]
    fn thread_local_is_per_thread() {
        // Set verbose on the main thread; a spawned thread should not see it.
        install(Diagnostics::new(true));
        let other = std::thread::spawn(|| current().verbose()).join().unwrap();
        assert!(
            !other,
            "spawned thread must see the silent default, not inherit from parent"
        );
    }
}
