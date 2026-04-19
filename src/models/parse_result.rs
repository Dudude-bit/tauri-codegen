use super::TauriCommand;

/// Commands accumulated from scanning every source file.
/// (Types and enums are walked separately through `pipeline::collect`, so the
/// parse phase only needs to carry the command list.)
#[derive(Debug, Default)]
pub struct ParseResult {
    pub commands: Vec<TauriCommand>,
}

impl ParseResult {
    pub fn new() -> Self {
        Self::default()
    }
}
