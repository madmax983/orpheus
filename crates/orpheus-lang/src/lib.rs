//! Language and REPL surface for Orpheus.

pub(crate) mod ascii_roll;
mod ast;
mod builtins;
mod diagnostics;
mod eval;
pub(crate) mod export;
pub(crate) mod html;
mod loader;
mod midi_input;
pub(crate) mod mixer;
mod parser;
mod pitch;
mod repl;
pub(crate) mod session;
pub(crate) mod srt;
pub(crate) mod stats;
mod svg;
pub(crate) mod tracker;
mod tui;
pub(crate) mod txt;
mod types;
mod value;

pub use ascii_roll::render_ascii_roll;
pub use ast::{Expr, Module, Stmt};
pub use diagnostics::{LoadError, ParseError, TypeError};
pub use eval::{EvalError, eval_module, render_span};
pub use export::{
    RenderError, export_number_pattern_to_csv, export_number_pattern_to_json,
    export_number_pattern_to_md, export_sample_pattern_to_csv, export_sample_pattern_to_json,
    export_sample_pattern_to_md, render_sample_pattern_to_file,
    render_sample_pattern_to_file_with_bank, render_sample_pattern_to_wav,
};
pub use html::{export_number_pattern_to_html, export_sample_pattern_to_html};
pub use loader::load_file_strict;
pub use parser::parse_module;
pub use repl::{run_stdio, run_stdio_with_engine, run_stdio_with_engine_and_path};
pub use srt::{export_number_pattern_to_srt, export_sample_pattern_to_srt};
pub use stats::{number_pattern_stats, sample_pattern_stats};
pub use svg::{export_number_pattern_to_svg, export_sample_pattern_to_svg};
pub use tracker::{export_number_pattern_to_tracker, export_sample_pattern_to_tracker};
pub use tui::{render_initial_frame_for_test, run_with_engine, run_with_engine_and_path};
pub use txt::{export_number_pattern_to_txt, export_sample_pattern_to_txt};
pub use types::{Type, TypedModule, infer_module};
pub use value::{NumberPatternValue, SampleEvent, SamplePatternValue, Value};

// Hidden re-exports keep rustdoc examples for internal helpers compiling.
#[doc(hidden)]
pub use builtins::{apply_builtin_function, builtin_value, is_sample_identifier, stack_values};
#[doc(hidden)]
pub use eval::{apply_function_value, eval_into_bindings, f64_to_rational};
#[doc(hidden)]
pub use export::escape_json_string;
#[doc(hidden)]
pub use loader::load_file_runtime_strict;
#[doc(hidden)]
pub use pitch::{PitchLiteralError, parse_named_pitch_literal};
#[doc(hidden)]
pub use session::{MixerView, ReplSession, TransportView};
#[doc(hidden)]
pub use value::FunctionValue;

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
