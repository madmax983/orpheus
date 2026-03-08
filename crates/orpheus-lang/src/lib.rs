//! Language and REPL surface for Orpheus.

/// REPL type-checking mode for the bootstrap workspace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplMode {
    /// Uses permissive type behavior intended for interactive work.
    Loose,
    /// Uses strict type behavior intended for durable artifacts.
    Strict,
}
