//! Language and REPL surface for Orpheus.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplMode {
    Loose,
    Strict,
}
