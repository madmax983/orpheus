//! Language and REPL surface for Orpheus.

pub(crate) mod abc_export;
pub(crate) mod arduino_export;
pub(crate) mod ascii_roll;
mod ast;
mod builtins;
mod diagnostics;
mod error;
mod eval;
pub(crate) mod explain;
pub(crate) mod export;
#[cfg(feature = "experimental-gcode")]
pub(crate) mod gcode_export;
pub(crate) mod guitar_tab_export;
pub(crate) mod html;
#[cfg(feature = "lilypond_export")]
pub(crate) mod lilypond_export;
mod loader;
pub(crate) mod midi_export;
mod midi_input;
pub(crate) mod mixer;
pub(crate) mod number_roll;
pub mod orca;
pub(crate) mod osu_export;
mod parser;
mod pedal;
mod pitch;
mod repl;
pub(crate) mod scad_export;
pub(crate) mod scl;
pub(crate) mod session;
pub(crate) mod srt;
pub(crate) mod stats;
pub(crate) mod supercollider_export;
mod svg;
pub(crate) mod tracker;
mod tui;
pub(crate) mod txt;
mod types;
mod value;

pub use abc_export::export_number_pattern_to_abc;
pub use arduino_export::export_number_pattern_to_arduino;
pub use ascii_roll::render_ascii_roll;
pub use ast::{BinaryOp, Expr, GraphBinding, Module, Stmt};
pub use diagnostics::{LoadError, ParseError, TypeError};
pub use error::EvalError;
pub use eval::{eval_module, render_span};
pub use export::{
    RenderError, export_number_pattern_to_csv, export_number_pattern_to_json,
    export_number_pattern_to_md, export_sample_pattern_to_csv, export_sample_pattern_to_json,
    export_sample_pattern_to_md, render_sample_pattern_to_file,
    render_sample_pattern_to_file_with_bank, render_sample_pattern_to_wav,
};
#[cfg(feature = "experimental-gcode")]
pub use gcode_export::export_number_pattern_to_gcode;
pub use guitar_tab_export::export_number_pattern_to_guitar_tab;
pub use html::{export_number_pattern_to_html, export_sample_pattern_to_html};
#[cfg(feature = "lilypond_export")]
pub use lilypond_export::export_number_pattern_to_lilypond;
pub use loader::{StrictLoadedFile, load_file_strict};
pub use midi_export::{export_number_pattern_to_midi, export_sample_pattern_to_midi};
pub use number_roll::render_ascii_number_roll;
pub use osu_export::export_sample_pattern_to_osu;
pub use parser::parse_module;
pub use pedal::{
    PedalGraph, PedalValue, SignalKind, ValidatedPedalBinding, ValidatedPedalNode,
    ValidatedPedalPlan,
};
pub use repl::{run_stdio, run_stdio_with_engine, run_stdio_with_engine_and_path};
pub use scad_export::export_number_pattern_to_scad;
pub use srt::{export_number_pattern_to_srt, export_sample_pattern_to_srt};
pub use stats::{number_pattern_stats, sample_pattern_stats, tuning_stats};
pub use supercollider_export::{
    export_number_pattern_to_supercollider, export_sample_pattern_to_supercollider,
};
pub use svg::{export_number_pattern_to_svg, export_sample_pattern_to_svg};
pub use tracker::{export_number_pattern_to_tracker, export_sample_pattern_to_tracker};
pub use tui::{render_initial_frame_for_test, run_with_engine, run_with_engine_and_path};
pub use txt::{export_number_pattern_to_txt, export_sample_pattern_to_txt};

pub(crate) mod dot_export;
pub use dot_export::export_pedal_value_to_dot;
pub use scl::{SclError, parse_scala_file, parse_scala_source};
pub use types::{
    Type, TypeEnv, TypeScheme, TypeVarId, TypedModule, infer_into_bindings, infer_module,
};
pub use value::{
    ArpDirectionValue, BuiltinFn, BuiltinKind, GatePatternValue, NumberPatternValue,
    PitchClassSetValue, PluginPatternValue, SampleEvent, SamplePatternValue, TuningValue, UserFn,
    Value,
};

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
pub use tui::style::{
    UiTransportState, binding_legend_item, binding_list_item, format_cycle_position,
    format_tempo_bpm, format_transport_status, help_overlay_border_style,
    help_overlay_footer_style, key_legend_style, live_binding_style, pending_binding_style,
    routing_status_line, should_show_binding_legend, transport_state, transport_status_line,
    transport_status_style,
};
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

pub(crate) mod mermaid;
pub use mermaid::export_sample_pattern_to_mermaid_gantt;
