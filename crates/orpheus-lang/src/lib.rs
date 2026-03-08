//! Language and REPL surface for Orpheus.

mod ast;
mod builtins;
mod diagnostics;
mod eval;
mod parser;
mod value;

pub use ast::{Expr, Module, Stmt};
pub use diagnostics::ParseError;
pub use eval::{EvalError, eval_module};
pub use parser::parse_module;
pub use value::{NumberPatternValue, SampleEvent, SamplePatternValue, Value};

/// REPL type-checking mode for the bootstrap workspace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplMode {
    /// Uses permissive type behavior intended for interactive work.
    ///
    /// Task 5 still reports unresolved identifiers as eval errors because the
    /// placeholder playback fallback has not been implemented yet.
    Loose,
    /// Uses strict type behavior intended for durable artifacts.
    Strict,
}
