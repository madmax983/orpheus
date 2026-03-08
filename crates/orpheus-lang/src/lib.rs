//! Language and REPL surface for Orpheus.

mod ast;
mod diagnostics;
mod parser;

pub use ast::{Expr, Module, Stmt};
pub use diagnostics::ParseError;
pub use parser::parse_module;

/// REPL type-checking mode for the bootstrap workspace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplMode {
    /// Uses permissive type behavior intended for interactive work.
    Loose,
    /// Uses strict type behavior intended for durable artifacts.
    Strict,
}
