//! Language and REPL surface for Orpheus.

mod ascii_roll;
mod ast;
mod builtins;
mod diagnostics;
mod eval;
mod export;
mod html;
mod loader;
pub(crate) mod mixer;
mod parser;
mod pitch;
mod repl;
mod session;
mod stats;
mod svg;
mod tui;
mod types;
mod value;

pub use ascii_roll::render_ascii_roll;
pub use ast::{Expr, Module, Stmt};
pub use diagnostics::{LoadError, ParseError, TypeError};
pub use eval::{EvalError, eval_module, render_span};
pub use export::{
    RenderError, export_number_pattern_to_csv, export_number_pattern_to_json,
    export_sample_pattern_to_csv, export_sample_pattern_to_json, render_sample_pattern_to_file,
    render_sample_pattern_to_file_with_bank, render_sample_pattern_to_wav,
};
pub use html::{export_number_pattern_to_html, export_sample_pattern_to_html};
pub use loader::load_file_strict;
pub use parser::parse_module;
pub use repl::{run_stdio, run_stdio_with_engine, run_stdio_with_engine_and_path};
pub use stats::{number_pattern_stats, sample_pattern_stats};
pub use svg::{export_number_pattern_to_svg, export_sample_pattern_to_svg};
pub use tui::{render_initial_frame_for_test, run_with_engine, run_with_engine_and_path};
pub use types::{Type, TypedModule, infer_module};
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
