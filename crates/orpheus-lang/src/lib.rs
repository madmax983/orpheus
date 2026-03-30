//! Language and REPL surface for Orpheus.

mod ast;
mod builtins;
mod diagnostics;
mod eval;
pub mod export;
mod loader;
mod parser;
pub mod repl;
pub mod session;
mod svg;
pub mod tui;
mod types;
mod value;

pub use ast::{Expr, Module, Stmt};
pub use diagnostics::{LoadError, ParseError, TypeError};
pub use eval::{EvalError, eval_module, render_span};
pub use export::{
    RenderError, export_number_pattern_to_csv, export_number_pattern_to_json,
    export_sample_pattern_to_csv, export_sample_pattern_to_json, render_sample_pattern_to_file,
    render_sample_pattern_to_file_with_bank, render_sample_pattern_to_wav,
};
pub use loader::load_file_strict;
pub use parser::parse_module;
pub use svg::export_sample_pattern_to_svg;
pub use types::{Type, TypedModule, infer_into_bindings, infer_module};
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
