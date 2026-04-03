//! The `session` module manages the interactive state of an Orpheus environment.
//!
//! This module forms the bridge between the textual inputs of the user (via the REPL or TUI)
//! and the executing backend, managing the bindings of variables, the loading of external
//! sample banks, and real-time DSP commands (like tempo changes or transport control).
//!
//! The central type is `ReplSession`, which maintains a `BTreeMap` of variable names to
//! typed Orpheus [`Value`]s and interfaces directly with the `orpheus_dsp` layer via an
//! `EngineHandle`.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use orpheus_dsp::{
    EngineCommand, EngineHandle, PatternUpdate, SampleBank, TransportSnapshot,
    load_sample_bank_from_directory, render_routing_snapshot_to_stem_wavs,
};
use orpheus_pattern::Rational;

use crate::eval::eval_into_bindings;
use crate::export::render_sample_pattern_to_file_with_bank;
use crate::export::sample_trigger_from_event;
use crate::loader::load_file_runtime_strict;
use crate::midi_input;
use crate::mixer::MixerState;
use crate::types::infer_into_bindings;
use crate::{ReplMode, Type, Value};

/// Represents the interactive state of an Orpheus environment.
///
/// A `ReplSession` manages user bindings, loaded sample banks, and real-time DSP
/// commands. It acts as the bridge between textual inputs and the underlying audio engine.
///
/// ## Examples
///
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
///
/// // Create a new session linked to a stubbed audio engine (for testing).
/// let engine = EngineHandle::stub();
/// let mut session = ReplSession::with_engine(engine);
///
/// // Evaluate a simple pattern binding.
/// let result = session.eval_line("drums = bd sn");
/// assert!(result.is_ok());
/// ```
pub struct ReplSession {
    mode: ReplMode,
    engine: EngineHandle,
    sample_bank: SampleBank,
    sample_directory: Option<PathBuf>,
    bindings: BTreeMap<String, Value>,
    type_bindings: BTreeMap<String, Type>,
    mixer: MixerState,
    pattern_display: RefCell<PatternDisplayState>,
    midi_output: MidiOutputState,
    midi_input: MidiInputState,
    midi_note_mappings: HashMap<u8, String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PatternDisplayState {
    active_pattern_name: Option<String>,
    pending_pattern_name: Option<String>,
    pending_enqueued_after_publish: Option<u64>,
    last_loaded_pattern_name: Option<String>,
}

#[derive(Default)]
struct MidiOutputState {
    connection: Option<Arc<Mutex<MidiOutputConnection>>>,
    port_name: Option<String>,
}

#[derive(Default)]
struct MidiInputState {
    connection: Option<MidiInputConnection<()>>,
    port_name: Option<String>,
}

/// A snapshot of the transport state formatted for visual presentation.
///
/// `TransportView` encapsulates the underlying engine's `TransportSnapshot` and adds
/// presentation-level details, such as the names of the currently active and pending patterns.
///
/// ## Examples
///
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
///
/// let session = ReplSession::with_engine(EngineHandle::stub());
/// let view = session.transport_view();
///
/// assert!(view.active_pattern_name().is_none());
/// assert!(view.pending_pattern_name().is_none());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportView {
    snapshot: TransportSnapshot,
    active_pattern_name: Option<String>,
    pending_pattern_name: Option<String>,
}

/// A snapshot of the mixer routing state formatted for visual presentation.
///
/// `MixerView` encapsulates a summary of the currently active tracks and buses,
/// along with a flag indicating whether routing updates are pending execution
/// at the next cycle boundary.
///
/// ## Examples
///
/// ```
/// use orpheus_lang::ReplSession;
/// use orpheus_dsp::EngineHandle;
///
/// let mut session = ReplSession::with_engine(EngineHandle::stub());
/// session.eval_line(":track new drums").unwrap();
/// session.render_test_block_for_tui(1);
///
/// let view = session.mixer_view();
/// assert!(view.tracks().iter().any(|line| line.contains("drums -> <unbound>")));
/// assert!(view.buses().is_empty());
/// assert!(view.has_pending_routing());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixerView {
    tracks: Vec<String>,
    buses: Vec<String>,
    has_pending_routing: bool,
}

impl TransportView {
    /// Returns a reference to the underlying DSP transport snapshot.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let session = ReplSession::with_engine(EngineHandle::stub());
    /// let view = session.transport_view();
    /// let snapshot = view.snapshot();
    /// assert_eq!(snapshot.tempo_bpm(), 120.0);
    /// ```
    #[must_use]
    pub const fn snapshot(&self) -> &TransportSnapshot {
        &self.snapshot
    }

    /// Returns the name of the currently active (playing) pattern, if any.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// session.eval_line("drums = bd sn").unwrap();
    /// // Fast-forward transport to activate pattern
    /// session.render_test_block_for_tui(256);
    ///
    /// let view = session.transport_view();
    /// assert_eq!(view.active_pattern_name(), Some("drums"));
    /// ```
    #[must_use]
    pub fn active_pattern_name(&self) -> Option<&str> {
        self.active_pattern_name.as_deref()
    }

    /// Returns the name of the pattern pending execution at the next cycle boundary, if any.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// session.eval_line("drums = bd sn").unwrap();
    ///
    /// let view = session.transport_view();
    /// assert_eq!(view.pending_pattern_name(), Some("drums"));
    /// ```
    #[must_use]
    pub fn pending_pattern_name(&self) -> Option<&str> {
        self.pending_pattern_name.as_deref()
    }
}

impl MixerView {
    /// Returns a slice of strings summarizing the state of all active tracks.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// session.eval_line(":track new drums").unwrap();
    ///
    /// let view = session.mixer_view();
    /// let tracks = view.tracks();
    /// assert!(tracks.iter().any(|line| line.contains("drums -> <unbound>")));
    /// ```
    #[must_use]
    pub fn tracks(&self) -> &[String] {
        &self.tracks
    }

    /// Returns a slice of strings summarizing the state of all active buses.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// session.eval_line(":bus new verb").unwrap();
    ///
    /// let view = session.mixer_view();
    /// let buses = view.buses();
    /// assert!(buses.iter().any(|line| line.contains("verb -> master")));
    /// ```
    #[must_use]
    pub fn buses(&self) -> &[String] {
        &self.buses
    }

    /// Returns `true` if there are pending routing changes queued for the next cycle boundary.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// session.eval_line(":track new drums").unwrap();
    /// session.render_test_block_for_tui(1);
    ///
    /// assert!(session.mixer_view().has_pending_routing());
    /// ```
    #[must_use]
    pub const fn has_pending_routing(&self) -> bool {
        self.has_pending_routing
    }
}

impl ReplSession {
    #[cfg(test)]
    fn new() -> Self {
        Self::with_engine(EngineHandle::stub())
    }

    /// Creates a new `ReplSession` associated with the provided engine handle.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let engine = EngineHandle::stub();
    /// let session = ReplSession::with_engine(engine);
    /// ```
    pub fn with_engine(engine: EngineHandle) -> Self {
        Self {
            mode: ReplMode::Loose,
            engine,
            sample_bank: SampleBank::load_builtin(),
            sample_directory: None,
            bindings: BTreeMap::new(),
            type_bindings: BTreeMap::new(),
            mixer: MixerState::default(),
            pattern_display: RefCell::new(PatternDisplayState::default()),
            midi_output: MidiOutputState::default(),
            midi_input: MidiInputState::default(),
            midi_note_mappings: HashMap::new(),
        }
    }

    /// Evaluates a line of input, updating the session's bindings or executing commands.
    ///
    /// The input can be a variable binding (e.g., `drums = bd sn`) or a REPL
    /// command starting with a colon (e.g., `:tempo 120`).
    ///
    /// ## Errors
    ///
    /// Returns an `Err` containing a descriptive message if the input fails to parse,
    /// type-check, evaluate, or if a REPL command is invalid.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    ///
    /// // Bind a pattern.
    /// let response = session.eval_line("notes = 1 2 3").unwrap();
    /// assert_eq!(response, "bound notes: Pattern<Number>");
    ///
    /// // Execute a command.
    /// let response = session.eval_line(":tempo 120").unwrap();
    /// assert_eq!(response, "tempo set to 120 BPM");
    /// ```
    pub fn eval_line(&mut self, source: &str) -> Result<String, String> {
        self.apply_midi_note_mappings()?;
        if source.starts_with(':') {
            return self.eval_command(source);
        }

        let Some((name, ty)) = infer_into_bindings(source, self.mode, &mut self.type_bindings)
            .map_err(|error| error.to_string())?
        else {
            return Err("no bindings were produced".into());
        };
        let Some((value_name, value)) = eval_into_bindings(source, self.mode, &mut self.bindings)
            .map_err(|error| error.to_string())?
        else {
            return Err("no bindings were produced".into());
        };
        debug_assert_eq!(name, value_name);

        self.push_pattern_update(&name, &value)?;
        Ok(success_banner(&name, &ty))
    }

    fn eval_command(&mut self, source: &str) -> Result<String, String> {
        let command = source.trim_start_matches(':').trim();
        if command.is_empty() {
            return Err("empty REPL command".to_owned());
        }
        let (name, args) = command
            .split_once(char::is_whitespace)
            .map_or((command, ""), |(name, args)| (name, args.trim()));

        match name {
            "render" => {
                if args.is_empty() {
                    Err(render_usage().to_owned())
                } else {
                    self.render_binding(args)
                }
            }
            "roll" => {
                if args.is_empty() {
                    Err(roll_usage().to_owned())
                } else {
                    self.roll_binding(args)
                }
            }
            "stats" => {
                if args.is_empty() {
                    Err(stats_usage().to_owned())
                } else {
                    self.stats_binding(args)
                }
            }
            "export" => {
                if args.is_empty() {
                    Err(export_usage().to_owned())
                } else if args.starts_with("stems") {
                    self.export_stems(args)
                } else {
                    self.export_binding(args)
                }
            }
            "tempo" => {
                if args.is_empty() {
                    Err(tempo_usage().to_owned())
                } else {
                    self.set_tempo(args)
                }
            }
            "samples" => {
                if args.is_empty() {
                    Err(samples_usage().to_owned())
                } else {
                    self.load_sample_directory(args)
                }
            }
            "open" => {
                if args.is_empty() {
                    Err(open_usage().to_owned())
                } else {
                    self.open_file(args)
                }
            }
            "track" => {
                if args.is_empty() {
                    Err(track_usage().to_owned())
                } else {
                    self.eval_track_command(args)
                }
            }
            "bus" => {
                if args.is_empty() {
                    Err(bus_usage().to_owned())
                } else {
                    self.eval_bus_command(args)
                }
            }
            "send" => {
                if args.is_empty() {
                    Err(send_usage().to_owned())
                } else {
                    self.eval_send_command(args)
                }
            }
            "mixer" => self.mixer_command(args),
            "midi" => self.midi_command(args),
            "reload-samples" => self.reload_sample_directory(args),
            "play" => self.play_transport(args),
            "stop" => self.stop_transport(args),
            other => Err(format!("unknown REPL command `:{other}`")),
        }
    }

    fn render_binding(&self, args: &str) -> Result<String, String> {
        let tokens = args.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 2 {
            return Err(render_usage().to_owned());
        }

        let cycles = if tokens.len() >= 3 {
            tokens
                .last()
                .and_then(|token| token.parse::<u64>().ok())
                .unwrap_or(1)
        } else {
            1
        };
        let path_end = if tokens.len() >= 3 && tokens.last().unwrap().parse::<u64>().is_ok() {
            tokens.len() - 1
        } else {
            tokens.len()
        };

        let binding_name = tokens[0];
        let path = tokens[1..path_end].join(" ");
        if path.is_empty() {
            return Err(render_usage().to_owned());
        }

        let value = self
            .bindings
            .get(binding_name)
            .ok_or_else(|| format!("no binding named `{binding_name}`"))?;
        let Value::SamplePattern(pattern) = value else {
            return Err(format!(
                "binding `{binding_name}` is not a sample pattern and cannot be rendered"
            ));
        };

        render_sample_pattern_to_file_with_bank(pattern, &path, cycles, &self.sample_bank)
            .map_err(|error: crate::export::RenderError| error.to_string())?;
        Ok(format!(
            "rendered `{binding_name}` to `{path}` ({cycles} cycle(s))"
        ))
    }

    fn roll_binding(&self, args: &str) -> Result<String, String> {
        let mut parts = args.split_whitespace();
        let binding_name = parts.next().ok_or_else(|| roll_usage().to_owned())?;

        let cycles = parts
            .next()
            .unwrap_or("1")
            .parse::<u64>()
            .map_err(|_| "cycles must be a positive integer".to_owned())?;

        let steps_per_cycle = parts
            .next()
            .unwrap_or("16")
            .parse::<u32>()
            .map_err(|_| "steps_per_cycle must be a positive integer".to_owned())?;

        if let Some(value) = self.bindings.get(binding_name) {
            match value {
                crate::value::Value::SamplePattern(pattern) => {
                    let roll = crate::ascii_roll::render_ascii_roll(
                        binding_name,
                        pattern,
                        cycles,
                        steps_per_cycle,
                    )
                    .map_err(|error| error.to_string())?;
                    Ok(format!("\n{}", roll.trim_end()))
                }
                _ => Err(format!(
                    "binding `{binding_name}` is a {} and cannot be rendered as a roll",
                    value.kind_name()
                )),
            }
        } else {
            Err(format!("no binding named `{binding_name}`"))
        }
    }

    fn stats_binding(&self, args: &str) -> Result<String, String> {
        let mut parts = args.split_whitespace();
        let binding_name = parts.next().ok_or_else(|| stats_usage().to_owned())?;

        let cycles = parts
            .next()
            .unwrap_or("1")
            .parse::<u64>()
            .map_err(|_| "cycles must be a positive integer".to_owned())?;

        if let Some(value) = self.bindings.get(binding_name) {
            match value {
                crate::value::Value::SamplePattern(pattern) => {
                    let stats = crate::stats::sample_pattern_stats(binding_name, pattern, cycles)
                        .map_err(|error| error.to_string())?;
                    Ok(format!("\n{}", stats.trim_end()))
                }
                crate::value::Value::NumberPattern(pattern) => {
                    let stats = crate::stats::number_pattern_stats(binding_name, pattern, cycles)
                        .map_err(|error| error.to_string())?;
                    Ok(format!("\n{}", stats.trim_end()))
                }
                _ => Err(format!(
                    "binding `{binding_name}` is a {} and cannot be analyzed",
                    value.kind_name()
                )),
            }
        } else {
            Err(format!("no binding named `{binding_name}`"))
        }
    }

    fn export_binding(&self, args: &str) -> Result<String, String> {
        let tokens = args.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 2 {
            return Err(export_usage().to_owned());
        }

        let cycles = if tokens.len() >= 3 {
            tokens
                .last()
                .and_then(|token| token.parse::<u64>().ok())
                .unwrap_or(1)
        } else {
            1
        };
        let path_end = if tokens.len() >= 3 && tokens.last().unwrap().parse::<u64>().is_ok() {
            tokens.len() - 1
        } else {
            tokens.len()
        };

        let binding_name = tokens[0];
        let path = tokens[1..path_end].join(" ");
        if path.is_empty() {
            return Err(export_usage().to_owned());
        }

        let value = self
            .bindings
            .get(binding_name)
            .ok_or_else(|| format!("no binding named `{binding_name}`"))?;

        Self::export_pattern_value(value, &path, cycles, binding_name)?;

        Ok(format!(
            "exported `{binding_name}` to `{path}` ({cycles} cycle(s))"
        ))
    }

    fn export_stems(&self, args: &str) -> Result<String, String> {
        let tokens = args.split_whitespace().collect::<Vec<_>>();
        if tokens.first().copied() != Some("stems") {
            return Err(export_usage().to_owned());
        }

        let mut cycles = 1_u64;
        let mut include_buses = false;
        for token in tokens.iter().skip(1) {
            if *token == "--buses" {
                include_buses = true;
            } else {
                cycles = token
                    .parse::<u64>()
                    .map_err(|_| "cycles must be a positive integer".to_owned())?;
            }
        }
        if cycles == 0 {
            return Err("cycles must be a positive integer".to_owned());
        }

        let snapshot = self.mixer.compile_snapshot(&self.bindings)?;
        let has_active_tracks = snapshot
            .tracks()
            .iter()
            .any(|track| !track.source().is_unbound() && !track.muted());
        if !has_active_tracks {
            return Err(
                "no active sample tracks to export; bind a sample pattern or create/bind tracks first"
                    .to_owned(),
            );
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_secs();
        let export_dir = PathBuf::from("exports").join(format!("stems-{timestamp}"));
        let tempo_bpm = self.transport_snapshot().tempo_bpm();
        let written = render_routing_snapshot_to_stem_wavs(
            &snapshot,
            cycles,
            tempo_bpm,
            &self.sample_bank,
            &export_dir,
            include_buses,
        )
        .map_err(|error| error.to_string())?;
        if written.is_empty() {
            return Err("no stems were written for the current routing state".to_owned());
        }

        Ok(format!(
            "exported {} stem(s) to `{}` ({cycles} cycle(s))",
            written.len(),
            export_dir.display()
        ))
    }

    fn export_pattern_value(
        value: &Value,
        path: &str,
        cycles: u64,
        binding_name: &str,
    ) -> Result<(), String> {
        match value {
            Value::SamplePattern(pattern) => {
                let export_path = std::path::Path::new(path);
                if export_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
                {
                    crate::svg::export_sample_pattern_to_svg(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                } else if export_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("html"))
                {
                    crate::html::export_sample_pattern_to_html(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                } else if export_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                {
                    crate::export::export_sample_pattern_to_json(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                } else if export_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
                {
                    crate::export::export_sample_pattern_to_md(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                } else if export_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
                {
                    crate::txt::export_sample_pattern_to_txt(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                } else if export_path.extension().is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("trk") || ext.eq_ignore_ascii_case("tracker")
                }) {
                    crate::tracker::export_sample_pattern_to_tracker(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                } else {
                    crate::export::export_sample_pattern_to_csv(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                }
            }
            Value::NumberPattern(pattern) => {
                let export_path = std::path::Path::new(path);
                if export_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
                {
                    crate::svg::export_number_pattern_to_svg(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                } else if export_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("html"))
                {
                    crate::html::export_number_pattern_to_html(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                } else if export_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("json"))
                {
                    crate::export::export_number_pattern_to_json(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                } else if export_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
                {
                    crate::export::export_number_pattern_to_md(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                } else if export_path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("txt"))
                {
                    crate::txt::export_number_pattern_to_txt(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                } else if export_path.extension().is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("trk") || ext.eq_ignore_ascii_case("tracker")
                }) {
                    crate::tracker::export_number_pattern_to_tracker(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                } else {
                    crate::export::export_number_pattern_to_csv(pattern, path, cycles)
                        .map_err(|error: crate::EvalError| error.to_string())?;
                }
            }
            Value::ArpDirection(_)
            | Value::PitchClassSet(_)
            | Value::Function(_)
            | Value::String(_) => {
                return Err(format!(
                    "binding `{binding_name}` is a {} and cannot be exported",
                    value.kind_name()
                ));
            }
        }
        Ok(())
    }

    fn set_tempo(&mut self, args: &str) -> Result<String, String> {
        let tokens = args.split_whitespace().collect::<Vec<_>>();
        if tokens.len() != 1 {
            return Err(tempo_usage().to_owned());
        }

        let tempo_bpm = tokens[0]
            .parse::<f32>()
            .map_err(|_| "tempo must be a finite positive BPM".to_owned())?;
        if !tempo_bpm.is_finite() || tempo_bpm <= 0.0 {
            return Err("tempo must be a finite positive BPM".to_owned());
        }

        self.engine
            .enqueue(EngineCommand::SetTempo(tempo_bpm))
            .map_err(|error| error.to_string())?;
        Ok(format!("tempo set to {tempo_bpm} BPM"))
    }

    fn load_sample_directory(&mut self, args: &str) -> Result<String, String> {
        let directory = PathBuf::from(args);
        let sample_bank =
            load_sample_bank_from_directory(&directory).map_err(|error| error.to_string())?;
        let available_tokens = sample_bank.available_tokens();
        self.sample_bank = sample_bank.clone();
        self.sample_directory = Some(directory.clone());
        self.engine
            .enqueue(EngineCommand::ReplaceSampleBank(sample_bank))
            .map_err(|error| error.to_string())?;
        Ok(format!(
            "loaded sample overrides from `{}` ({})",
            directory.display(),
            available_tokens.join(", ")
        ))
    }

    /// Loads an Orpheus source file, replacing the current session's bindings.
    ///
    /// The entire file is evaluated strictly. Any bindings produced by the file
    /// will replace the existing bindings in the session, and the mixer state
    /// will be reset.
    ///
    /// ## Errors
    ///
    /// Returns an `Err` if the file cannot be read, parsed, type-checked, or evaluated.
    ///
    /// ## Examples
    ///
    /// ```no_run
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// session.open_file("song.ode").unwrap();
    /// ```
    pub fn open_file(&mut self, path: impl AsRef<Path>) -> Result<String, String> {
        let path = path.as_ref();
        let loaded = load_file_runtime_strict(path).map_err(|error| error.to_string())?;
        let binding_names = loaded
            .type_bindings
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let last_binding_name = loaded.last_binding_name.clone();

        self.bindings = loaded.value_bindings;
        self.type_bindings = loaded.type_bindings;
        self.mixer = MixerState::default();
        *self.pattern_display.borrow_mut() = PatternDisplayState::default();

        if let Some(name) = last_binding_name {
            if let Some(value) = self.bindings.get(&name).cloned() {
                self.push_pattern_update(&name, &value)?;
            }
        }

        Ok(format!("opened `{}` ({binding_names})", path.display()))
    }

    fn reload_sample_directory(&mut self, args: &str) -> Result<String, String> {
        if !args.is_empty() {
            return Err(reload_samples_usage().to_owned());
        }

        let Some(directory) = self.sample_directory.clone() else {
            return Err("no sample directory has been configured".to_owned());
        };
        let sample_bank =
            load_sample_bank_from_directory(&directory).map_err(|error| error.to_string())?;
        let available_tokens = sample_bank.available_tokens();
        self.sample_bank = sample_bank.clone();
        self.engine
            .enqueue(EngineCommand::ReplaceSampleBank(sample_bank))
            .map_err(|error| error.to_string())?;
        Ok(format!(
            "reloaded sample overrides from `{}` ({})",
            directory.display(),
            available_tokens.join(", ")
        ))
    }

    fn play_transport(&mut self, args: &str) -> Result<String, String> {
        if !args.is_empty() {
            return Err(play_usage().to_owned());
        }

        self.engine
            .enqueue(EngineCommand::PlayTransport)
            .map_err(|error| format!("failed to enqueue play transport command: {error}"))?;
        Ok("transport playing".to_owned())
    }

    fn stop_transport(&mut self, args: &str) -> Result<String, String> {
        if !args.is_empty() {
            return Err(stop_usage().to_owned());
        }

        self.engine
            .enqueue(EngineCommand::StopTransport)
            .map_err(|error| format!("failed to enqueue stop transport command: {error}"))?;
        Ok("transport stopped".to_owned())
    }

    fn eval_track_command(&mut self, args: &str) -> Result<String, String> {
        let tokens = args.split_whitespace().collect::<Vec<_>>();
        let Some(subcommand) = tokens.first().copied() else {
            return Err(track_usage().to_owned());
        };

        match subcommand {
            "new" if tokens.len() == 2 => {
                let track_name = tokens[1];
                self.mixer.new_track(track_name)?;
                self.enqueue_mixer_snapshot()?;
                Ok(format!("created track `{track_name}`"))
            }
            "bind" if tokens.len() == 3 => {
                let track_name = tokens[1];
                let binding_name = tokens[2];
                self.mixer
                    .bind_track(track_name, binding_name, &self.bindings)?;
                self.enqueue_mixer_snapshot()?;
                Ok(format!("bound track `{track_name}` to `{binding_name}`"))
            }
            "level" if tokens.len() == 3 => {
                let track_name = tokens[1];
                let level = tokens[2]
                    .parse::<f32>()
                    .map_err(|_| "track level must be a finite value >= 0".to_owned())?;
                self.mixer.set_track_level(track_name, level)?;
                self.enqueue_mixer_snapshot()?;
                Ok(format!("set track `{track_name}` level to {level}"))
            }
            "mute" if tokens.len() == 3 => {
                let track_name = tokens[1];
                let muted = match tokens[2] {
                    "on" => true,
                    "off" => false,
                    _ => return Err("track mute expects `on` or `off`".to_owned()),
                };
                self.mixer.set_track_mute(track_name, muted)?;
                self.enqueue_mixer_snapshot()?;
                Ok(format!(
                    "track `{track_name}` mute {}",
                    if muted { "on" } else { "off" }
                ))
            }
            _ => Err(track_usage().to_owned()),
        }
    }

    fn eval_bus_command(&mut self, args: &str) -> Result<String, String> {
        let tokens = args.split_whitespace().collect::<Vec<_>>();
        match tokens.as_slice() {
            ["new", bus_name] => {
                self.mixer.new_bus(bus_name)?;
                self.enqueue_mixer_snapshot()?;
                Ok(format!("created bus `{bus_name}`"))
            }
            ["fx", bus_name, "none"] => {
                self.mixer.clear_bus_effect(bus_name)?;
                self.enqueue_mixer_snapshot()?;
                Ok(format!("cleared hosted effect on bus `{bus_name}`"))
            }
            ["fx", bus_name, "delay", params @ ..] => {
                let params = parse_bus_delay_params(params)?;
                self.mixer
                    .set_bus_delay(bus_name, params.time, params.feedback, params.wet)?;
                self.enqueue_mixer_snapshot()?;
                Ok(format!("attached delay to bus `{bus_name}`"))
            }
            ["fx", bus_name, "reverb", params @ ..] => {
                let params = parse_bus_reverb_params(params)?;
                self.mixer
                    .set_bus_reverb(bus_name, params.size, params.damp, params.wet)?;
                self.enqueue_mixer_snapshot()?;
                Ok(format!("attached reverb to bus `{bus_name}`"))
            }
            _ => Err(bus_usage().to_owned()),
        }
    }

    fn eval_send_command(&mut self, args: &str) -> Result<String, String> {
        let tokens = args.split_whitespace().collect::<Vec<_>>();
        match tokens.as_slice() {
            [track_name, bus_name, level] => {
                let level = level
                    .parse::<f32>()
                    .map_err(|_| "send level must be a finite value in [0, 1]".to_owned())?;
                self.mixer.set_send(track_name, bus_name, level)?;
                self.enqueue_mixer_snapshot()?;
                Ok(format!("sent `{track_name}` to `{bus_name}` at {level}"))
            }
            _ => Err(send_usage().to_owned()),
        }
    }

    fn mixer_command(&self, args: &str) -> Result<String, String> {
        if !args.is_empty() {
            return Err(mixer_usage().to_owned());
        }
        let summary = self.mixer.render_summary();
        if summary.is_empty() {
            Ok("mixer is empty".to_owned())
        } else {
            Ok(summary)
        }
    }

    fn midi_command(&mut self, args: &str) -> Result<String, String> {
        let tokens = args.split_whitespace().collect::<Vec<_>>();
        match tokens.as_slice() {
            ["in", "list"] => self.list_midi_inputs(),
            ["in", "connect", port @ ..] if !port.is_empty() => {
                self.connect_midi_input(&port.join(" "))
            }
            ["in", "disconnect"] => self.disconnect_midi_input(),
            ["in", "map-note", note, binding_name] => self.map_midi_note(note, binding_name),
            ["in", "unmap-note", note] => self.unmap_midi_note(note),
            ["list"] => self.list_midi_outputs(),
            ["connect", port @ ..] if !port.is_empty() => self.connect_midi_output(&port.join(" ")),
            ["disconnect"] => self.disconnect_midi_output(),
            ["send", binding_name] => self.send_midi_binding(binding_name, 1),
            ["send", binding_name, channel] => {
                let channel = channel
                    .parse::<u8>()
                    .map_err(|_| "MIDI channel must be an integer in [1, 16]".to_owned())?;
                self.send_midi_binding(binding_name, channel)
            }
            _ => Err(midi_usage().to_owned()),
        }
    }

    fn list_midi_inputs(&self) -> Result<String, String> {
        let midi_in = MidiInput::new("orpheus")
            .map_err(|error| format!("failed to initialize MIDI input subsystem: {error}"))?;
        let ports = midi_in.ports();
        let mut port_names = ports
            .iter()
            .map(|port| {
                midi_in
                    .port_name(port)
                    .unwrap_or_else(|_| "<unreadable port>".to_owned())
            })
            .collect::<Vec<_>>();
        port_names.sort_unstable();
        if port_names.is_empty() {
            Ok("available MIDI input ports: <none>".to_owned())
        } else {
            Ok(format!(
                "available MIDI input ports: {}",
                port_names.join(", ")
            ))
        }
    }

    fn connect_midi_input(&mut self, raw_port_name: &str) -> Result<String, String> {
        let port_name = trim_quoted_arg(raw_port_name).to_owned();
        if port_name.is_empty() {
            return Err(midi_usage().to_owned());
        }
        let mut midi_in = MidiInput::new("orpheus")
            .map_err(|error| format!("failed to initialize MIDI input subsystem: {error}"))?;
        midi_in.ignore(Ignore::None);
        let port = midi_in
            .ports()
            .into_iter()
            .find(|candidate| {
                midi_in
                    .port_name(candidate)
                    .map(|name| name == port_name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| format!("no MIDI input port named `{port_name}`"))?;
        let connection = midi_in
            .connect(
                &port,
                "orpheus-midi-in",
                move |_timestamp, message, ()| midi_input::update_from_message(message),
                (),
            )
            .map_err(|error| format!("failed to connect to MIDI input `{port_name}`: {error}"))?;
        self.midi_input.connection = Some(connection);
        self.midi_input.port_name = Some(port_name.clone());
        Ok(format!("connected MIDI input `{port_name}`"))
    }

    fn disconnect_midi_input(&mut self) -> Result<String, String> {
        let Some(port_name) = self.midi_input.port_name.take() else {
            return Err("no MIDI input is connected".to_owned());
        };
        self.midi_input.connection = None;
        Ok(format!("disconnected MIDI input `{port_name}`"))
    }

    fn map_midi_note(&mut self, raw_note: &str, binding_name: &str) -> Result<String, String> {
        let note = raw_note
            .parse::<u8>()
            .map_err(|_| "MIDI note must be an integer in [0, 127]".to_owned())?;
        if note > 127 {
            return Err("MIDI note must be an integer in [0, 127]".to_owned());
        }
        self.midi_note_mappings
            .insert(note, binding_name.to_owned());
        Ok(format!(
            "mapped MIDI note {note} to binding `{binding_name}`"
        ))
    }

    fn unmap_midi_note(&mut self, raw_note: &str) -> Result<String, String> {
        let note = raw_note
            .parse::<u8>()
            .map_err(|_| "MIDI note must be an integer in [0, 127]".to_owned())?;
        if self.midi_note_mappings.remove(&note).is_some() {
            Ok(format!("removed MIDI note mapping for {note}"))
        } else {
            Err(format!("no MIDI note mapping exists for {note}"))
        }
    }

    fn list_midi_outputs(&self) -> Result<String, String> {
        let midi_out = MidiOutput::new("orpheus")
            .map_err(|error| format!("failed to initialize MIDI output subsystem: {error}"))?;
        let ports = midi_out.ports();
        let mut port_names = ports
            .iter()
            .map(|port| {
                midi_out
                    .port_name(port)
                    .unwrap_or_else(|_| "<unreadable port>".to_owned())
            })
            .collect::<Vec<_>>();
        port_names.sort_unstable();
        if port_names.is_empty() {
            Ok("available MIDI output ports: <none>".to_owned())
        } else {
            Ok(format!(
                "available MIDI output ports: {}",
                port_names.join(", ")
            ))
        }
    }

    fn connect_midi_output(&mut self, raw_port_name: &str) -> Result<String, String> {
        let port_name = trim_quoted_arg(raw_port_name).to_owned();
        if port_name.is_empty() {
            return Err(midi_usage().to_owned());
        }
        let midi_out = MidiOutput::new("orpheus")
            .map_err(|error| format!("failed to initialize MIDI output subsystem: {error}"))?;
        let port = midi_out
            .ports()
            .into_iter()
            .find(|candidate| {
                midi_out
                    .port_name(candidate)
                    .map(|name| name == port_name)
                    .unwrap_or(false)
            })
            .ok_or_else(|| format!("no MIDI output port named `{port_name}`"))?;
        let connection = midi_out
            .connect(&port, "orpheus-midi-out")
            .map_err(|error| format!("failed to connect to MIDI output `{port_name}`: {error}"))?;
        self.midi_output.connection = Some(Arc::new(Mutex::new(connection)));
        self.midi_output.port_name = Some(port_name.clone());
        Ok(format!("connected MIDI output `{port_name}`"))
    }

    fn disconnect_midi_output(&mut self) -> Result<String, String> {
        let Some(port_name) = self.midi_output.port_name.take() else {
            return Err("no MIDI output is connected".to_owned());
        };
        self.midi_output.connection = None;
        Ok(format!("disconnected MIDI output `{port_name}`"))
    }

    fn send_midi_binding(&mut self, binding_name: &str, channel: u8) -> Result<String, String> {
        if !(1..=16).contains(&channel) {
            return Err("MIDI channel must be an integer in [1, 16]".to_owned());
        }
        let Some(connection) = self.midi_output.connection.clone() else {
            return Err("no MIDI output is connected; run `:midi connect <port>` first".to_owned());
        };
        let value = self
            .bindings
            .get(binding_name)
            .ok_or_else(|| format!("no binding named `{binding_name}`"))?;
        let Value::NumberPattern(pattern) = value else {
            return Err(format!(
                "binding `{binding_name}` is a {} and cannot be sent as MIDI notes",
                value.kind_name()
            ));
        };

        let mut midi_events = Vec::new();
        for event in pattern.query_unit() {
            let note = event.value.round().clamp(0.0, 127.0) as u8;
            let start = f64::from(event.part.start());
            let end = f64::from(event.part.end());
            if end > start {
                midi_events.push((start, true, note));
                midi_events.push((end, false, note));
            }
        }
        midi_events.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| right.1.cmp(&left.1))
                .then_with(|| left.2.cmp(&right.2))
        });

        let event_count = midi_events.len();
        let transport = self.transport_snapshot();
        let seconds_per_cycle = 240.0 / f64::from(transport.tempo_bpm());
        let status_base = 0x90_u8 + (channel - 1);
        thread::spawn(move || {
            let start = Instant::now();
            for (offset_in_cycle, note_on, note) in midi_events {
                let target_time = Duration::from_secs_f64(offset_in_cycle * seconds_per_cycle);
                let elapsed = start.elapsed();
                if target_time > elapsed {
                    thread::sleep(target_time.checked_sub(elapsed).unwrap());
                }
                let status = if note_on {
                    status_base
                } else {
                    status_base - 0x10
                };
                let velocity = if note_on { 100 } else { 0 };
                if let Ok(mut guard) = connection.lock() {
                    let _ = guard.send(&[status, note, velocity]);
                }
            }
        });

        Ok(format!(
            "queued {event_count} MIDI events from `{binding_name}` on channel {channel}"
        ))
    }

    fn apply_midi_note_mappings(&mut self) -> Result<(), String> {
        for event in midi_input::drain_note_events() {
            if event.kind != midi_input::MidiNoteEventKind::On {
                continue;
            }
            let Some(binding_name) = self.midi_note_mappings.get(&event.note).cloned() else {
                continue;
            };
            let Some(value) = self.bindings.get(&binding_name).cloned() else {
                continue;
            };
            self.push_pattern_update(&binding_name, &value)?;
        }
        Ok(())
    }

    fn enqueue_mixer_snapshot(&mut self) -> Result<(), String> {
        let snapshot = self.mixer.compile_snapshot(&self.bindings)?;
        self.engine
            .enqueue(EngineCommand::SwapRoutingSnapshot(snapshot))
            .map_err(|error| format!("failed to enqueue routing snapshot swap: {error}"))?;

        if self.mixer.has_explicit_bound_tracks() {
            let mut display = self.pattern_display.borrow_mut();
            display.active_pattern_name = None;
            display.pending_pattern_name = None;
            display.pending_enqueued_after_publish = None;
        }

        Ok(())
    }

    fn push_pattern_update(&mut self, name: &str, value: &Value) -> Result<(), String> {
        if let Value::SamplePattern(pattern) = value {
            self.mixer.note_sample_binding(name);
            if self.mixer.has_routing_state() {
                self.pattern_display.borrow_mut().last_loaded_pattern_name = Some(name.to_owned());
                return self.enqueue_mixer_snapshot();
            }

            let enqueue_publish = self.engine.transport_snapshot().publish_epoch();
            let events = pattern.query_unit().map_err(|error| {
                format!("failed to query unit span for publishing pattern `{name}`: {error}")
            })?;
            let update = PatternUpdate::new(
                name,
                events
                    .into_iter()
                    .map(|event| orpheus_pattern::Event {
                        whole: event.whole,
                        part: event.part,
                        value: sample_trigger_from_event(&event.value),
                    })
                    .collect(),
            );
            self.engine
                .enqueue(EngineCommand::LoadPattern(update))
                .map_err(|error| {
                    format!("failed to enqueue load pattern command for `{name}`: {error}")
                })?;
            let mut display = self.pattern_display.borrow_mut();
            if display.active_pattern_name.is_none() && enqueue_publish != 0 {
                if let Some(last_loaded_pattern_name) = display.last_loaded_pattern_name.clone() {
                    display.active_pattern_name = Some(last_loaded_pattern_name);
                }
            }
            display.last_loaded_pattern_name = Some(name.to_owned());
            display.pending_pattern_name = Some(name.to_owned());
            display.pending_enqueued_after_publish = Some(enqueue_publish);
        }

        Ok(())
    }

    /// Returns a summary of all active bindings and their inferred types.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// session.eval_line("notes = 1 2").unwrap();
    /// session.eval_line("drums = bd sn").unwrap();
    ///
    /// let summaries = session.binding_summaries();
    /// assert_eq!(summaries, vec!["drums: Pattern<Sample>", "notes: Pattern<Number>"]);
    /// ```
    pub fn binding_summaries(&self) -> Vec<String> {
        self.type_bindings
            .iter()
            .map(|(name, ty)| format!("{name}: {ty}"))
            .collect()
    }

    #[cfg(test)]
    pub fn last_loaded_pattern_name(&self) -> Option<String> {
        self.pattern_display
            .borrow()
            .last_loaded_pattern_name
            .clone()
    }

    /// Captures a point-in-time snapshot of the underlying audio engine's transport state.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let session = ReplSession::with_engine(EngineHandle::stub());
    /// let snapshot = session.transport_snapshot();
    /// assert_eq!(snapshot.tempo_bpm(), 120.0);
    /// ```
    pub fn transport_snapshot(&self) -> TransportSnapshot {
        self.transport_view().snapshot
    }

    /// Generates a structured view of the transport state, including visual details
    /// such as the currently active and pending pattern names.
    ///
    /// This is typically used by the TUI to render the transport overlay.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let session = ReplSession::with_engine(EngineHandle::stub());
    /// let view = session.transport_view();
    /// assert!(view.active_pattern_name().is_none());
    /// ```
    pub fn transport_view(&self) -> TransportView {
        let snapshot = self.engine.transport_snapshot();
        let mut display = self.pattern_display.borrow_mut();
        if let Some(pending_name) = display.pending_pattern_name.clone() {
            let enqueued_after_publish = display
                .pending_enqueued_after_publish
                .unwrap_or_else(|| snapshot.publish_epoch());
            if !snapshot.has_pending_pattern() && snapshot.publish_epoch() != enqueued_after_publish
            {
                display.active_pattern_name = Some(pending_name);
                display.pending_pattern_name = None;
                display.pending_enqueued_after_publish = None;
            }
        } else if display.active_pattern_name.is_none() && snapshot.current_frame() != 0 {
            if let Some(last_loaded_pattern_name) = display.last_loaded_pattern_name.clone() {
                display.active_pattern_name = Some(last_loaded_pattern_name);
            }
        }

        TransportView {
            snapshot,
            active_pattern_name: display.active_pattern_name.clone(),
            pending_pattern_name: display.pending_pattern_name.clone(),
        }
    }

    /// Generates a structured view of the current mixer state, detailing active
    /// tracks, buses, and pending routing changes.
    ///
    /// This is typically used by the TUI to render the mixer panel.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// session.eval_line(":track new drums").unwrap();
    ///
    /// let view = session.mixer_view();
    /// assert!(view.tracks().iter().any(|line| line.contains("drums -> <unbound>")));
    /// ```
    pub fn mixer_view(&self) -> MixerView {
        let snapshot = self.engine.transport_snapshot();
        MixerView {
            tracks: self.mixer.track_summary_lines(),
            buses: self.mixer.bus_summary_lines(),
            has_pending_routing: snapshot.has_pending_routing(),
        }
    }

    #[doc(hidden)]
    pub fn render_test_block_for_tui(&mut self, frames: u64) -> Vec<f32> {
        let _ = self.apply_midi_note_mappings();
        self.engine.render_test_block(frames)
    }

    #[doc(hidden)]
    pub fn frames_until_boundary_for_tui(&self) -> u64 {
        self.engine.frames_until_boundary_for_test()
    }
}

fn success_banner(name: &str, ty: &Type) -> String {
    format!("bound {name}: {ty}")
}

const fn render_usage() -> &'static str {
    "usage: :render <binding> <path> [cycles]"
}

const fn export_usage() -> &'static str {
    "usage: :export <binding> <path> [cycles] | :export stems [cycles] [--buses]"
}

const fn roll_usage() -> &'static str {
    "usage: :roll <binding> [cycles] [steps_per_cycle]"
}

const fn stats_usage() -> &'static str {
    "usage: :stats <binding> [cycles]"
}

const fn tempo_usage() -> &'static str {
    "usage: :tempo <bpm>"
}

const fn samples_usage() -> &'static str {
    "usage: :samples <directory>"
}

const fn track_usage() -> &'static str {
    "usage: :track <new|bind|level|mute> ..."
}

const fn bus_usage() -> &'static str {
    "usage: :bus new <name> | :bus fx <bus> delay time=<num>/<den> feedback=<f> wet=<f> | :bus fx <bus> reverb size=<f> damp=<f> wet=<f> | :bus fx <bus> none"
}

const fn send_usage() -> &'static str {
    "usage: :send <track> <bus> <level>"
}

const fn mixer_usage() -> &'static str {
    "usage: :mixer"
}

const fn midi_usage() -> &'static str {
    "usage: :midi <list|connect <port>|disconnect|send <binding> [channel]|in list|in connect <port>|in disconnect|in map-note <note> <binding>|in unmap-note <note>>"
}

const fn open_usage() -> &'static str {
    "usage: :open <path>"
}

const fn reload_samples_usage() -> &'static str {
    "usage: :reload-samples"
}

const fn play_usage() -> &'static str {
    "usage: :play"
}

const fn stop_usage() -> &'static str {
    "usage: :stop"
}

struct BusDelayParams {
    time: Rational,
    feedback: f32,
    wet: f32,
}

struct BusReverbParams {
    size: f32,
    damp: f32,
    wet: f32,
}

fn parse_bus_delay_params(tokens: &[&str]) -> Result<BusDelayParams, String> {
    let mut time = None;
    let mut feedback = None;
    let mut wet = None;

    for token in tokens {
        let (key, value) = token
            .split_once('=')
            .ok_or_else(|| format!("bus fx delay expects key=value arguments, got `{token}`"))?;
        match key {
            "time" => {
                time = Some(parse_rational_time(value)?);
            }
            "feedback" => {
                feedback =
                    Some(value.parse::<f32>().map_err(|_| {
                        "delay feedback must be a finite value in [0, 1]".to_owned()
                    })?);
            }
            "wet" => {
                wet = Some(
                    value
                        .parse::<f32>()
                        .map_err(|_| "delay wet must be a finite value in [0, 1]".to_owned())?,
                );
            }
            other => {
                return Err(format!("unknown bus fx delay key `{other}`"));
            }
        }
    }

    let time = time.ok_or_else(|| "bus fx delay requires time=<num>/<den>".to_owned())?;
    let feedback = feedback.ok_or_else(|| "bus fx delay requires feedback=<f>".to_owned())?;
    let wet = wet.ok_or_else(|| "bus fx delay requires wet=<f>".to_owned())?;
    Ok(BusDelayParams {
        time,
        feedback,
        wet,
    })
}

fn parse_bus_reverb_params(tokens: &[&str]) -> Result<BusReverbParams, String> {
    let mut size = None;
    let mut damp = None;
    let mut wet = None;

    for token in tokens {
        let (key, value) = token
            .split_once('=')
            .ok_or_else(|| format!("bus fx reverb expects key=value arguments, got `{token}`"))?;
        match key {
            "size" => {
                size = Some(
                    value
                        .parse::<f32>()
                        .map_err(|_| "reverb size must be a finite value in [0, 1]".to_owned())?,
                );
            }
            "damp" => {
                damp = Some(
                    value
                        .parse::<f32>()
                        .map_err(|_| "reverb damp must be a finite value in [0, 1]".to_owned())?,
                );
            }
            "wet" => {
                wet = Some(
                    value
                        .parse::<f32>()
                        .map_err(|_| "reverb wet must be a finite value in [0, 1]".to_owned())?,
                );
            }
            other => {
                return Err(format!("unknown bus fx reverb key `{other}`"));
            }
        }
    }

    let size = size.ok_or_else(|| "bus fx reverb requires size=<f>".to_owned())?;
    let damp = damp.ok_or_else(|| "bus fx reverb requires damp=<f>".to_owned())?;
    let wet = wet.ok_or_else(|| "bus fx reverb requires wet=<f>".to_owned())?;
    Ok(BusReverbParams { size, damp, wet })
}

fn parse_rational_time(value: &str) -> Result<Rational, String> {
    let (numerator, denominator) = value
        .split_once('/')
        .ok_or_else(|| "delay time must be a rational like 1/8".to_owned())?;
    let numerator = numerator
        .parse::<i64>()
        .map_err(|_| "delay time must be a rational like 1/8".to_owned())?;
    let denominator = denominator
        .parse::<i64>()
        .map_err(|_| "delay time must be a rational like 1/8".to_owned())?;
    Rational::new(numerator, denominator)
        .map_err(|_| "delay time must be a rational like 1/8".to_owned())
}

fn trim_quoted_arg(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::ReplSession;

    static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn temp_wav_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("orpheus-render-{}.wav", unique_temp_suffix()))
    }

    fn temp_csv_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("orpheus-export-{}.csv", unique_temp_suffix()))
    }

    fn temp_json_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("orpheus-export-{}.json", unique_temp_suffix()))
    }

    fn temp_svg_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("orpheus-export-{}.svg", unique_temp_suffix()))
    }

    #[test]
    fn eval_line_reuses_prior_bindings() {
        let mut session = ReplSession::new();

        assert_eq!(
            session.eval_line("drums = bd sn cp sn"),
            Ok("bound drums: Pattern<Sample>".to_owned())
        );
        assert_eq!(
            session.eval_line("copy = drums"),
            Ok("bound copy: Pattern<Sample>".to_owned())
        );
    }

    #[test]
    fn sample_patterns_drive_the_embedded_audio_engine() {
        let mut session = ReplSession::new();

        session.eval_line("drums = bd sn cp sn").unwrap();
        let rendered = session
            .engine
            .render_test_block(session.engine.frames_until_boundary_for_test() + 256);

        assert!(rendered.iter().any(|sample| sample.abs() > f32::EPSILON));
    }

    #[test]
    fn track_eval_line_keeps_using_main_track_by_default() {
        let mut session = ReplSession::new();

        session.eval_line("drums = bd sn").unwrap();
        let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());

        assert_eq!(session.engine.active_track_names_for_test(), ["main"]);
    }

    #[test]
    fn track_and_bus_commands_compile_a_routing_snapshot() {
        let mut session = ReplSession::new();

        session.eval_line("groove = bd sn").unwrap();
        assert_eq!(
            session.eval_line(":track new drums"),
            Ok("created track `drums`".to_owned())
        );
        assert_eq!(
            session.eval_line(":track bind drums groove"),
            Ok("bound track `drums` to `groove`".to_owned())
        );
        assert_eq!(
            session.eval_line(":bus new verb"),
            Ok("created bus `verb`".to_owned())
        );
        assert_eq!(
            session.eval_line(":send drums verb 0.35"),
            Ok("sent `drums` to `verb` at 0.35".to_owned())
        );

        let _ = session.render_test_block_for_tui(1);
        let snapshot = session.transport_snapshot();
        assert!(snapshot.has_pending_routing());

        let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
        assert_eq!(session.engine.active_track_names_for_test(), ["drums"]);
    }

    #[test]
    fn track_mixer_command_reports_track_assignments_and_sends() {
        let mut session = ReplSession::new();

        session.eval_line("groove = bd sn").unwrap();
        session.eval_line(":track new drums").unwrap();
        session.eval_line(":track bind drums groove").unwrap();
        session.eval_line(":bus new verb").unwrap();
        session.eval_line(":send drums verb 0.35").unwrap();

        session.eval_line(":bus new dub").unwrap();
        session.eval_line(":send drums dub 0.5").unwrap();

        let mixer = session.eval_line(":mixer").unwrap();

        assert!(mixer.contains("send verb @ 0.35"));
        assert!(mixer.contains("send dub @ 0.50"));
    }

    #[test]
    fn bus_fx_command_attaches_shared_delay_to_bus() {
        let mut session = ReplSession::new();

        session.eval_line(":bus new dub").unwrap();
        assert_eq!(
            session.eval_line(":bus fx dub delay time=3/16 feedback=0.45 wet=1.0"),
            Ok("attached delay to bus `dub`".to_owned())
        );

        let mixer = session.eval_line(":mixer").unwrap();
        assert!(mixer.contains("dub"));
        assert!(mixer.contains("delay(3/16"));
        let _ = session.render_test_block_for_tui(1);
        assert!(session.transport_snapshot().has_pending_routing());
    }

    #[test]
    fn bus_fx_command_attaches_shared_reverb_to_bus() {
        let mut session = ReplSession::new();

        session.eval_line(":bus new verb").unwrap();
        assert_eq!(
            session.eval_line(":bus fx verb reverb size=0.75 damp=0.35 wet=1.0"),
            Ok("attached reverb to bus `verb`".to_owned())
        );

        let mixer = session.eval_line(":mixer").unwrap();
        assert!(mixer.contains("verb"));
        assert!(mixer.contains("reverb(size=0.75 damp=0.35 wet=1.00)"));
        let _ = session.render_test_block_for_tui(1);
        assert!(session.transport_snapshot().has_pending_routing());
    }

    #[test]
    fn bus_fx_command_clears_hosted_effect_with_none() {
        let mut session = ReplSession::new();

        session.eval_line(":bus new dub").unwrap();
        session
            .eval_line(":bus fx dub delay time=3/16 feedback=0.45 wet=1.0")
            .unwrap();
        assert_eq!(
            session.eval_line(":bus fx dub none"),
            Ok("cleared hosted effect on bus `dub`".to_owned())
        );

        let mixer = session.eval_line(":mixer").unwrap();
        assert!(mixer.contains("dub"));
        assert!(!mixer.contains("delay("));
    }

    #[test]
    fn bus_fx_command_rejects_unknown_bus() {
        let mut session = ReplSession::new();

        assert_eq!(
            session.eval_line(":bus fx dub delay time=3/16 feedback=0.45 wet=1.0"),
            Err("no bus named `dub`".to_owned())
        );
    }

    #[test]
    fn bus_fx_command_rejects_bad_rational_time() {
        let mut session = ReplSession::new();
        session.eval_line(":bus new dub").unwrap();

        let error = session
            .eval_line(":bus fx dub delay time=bad feedback=0.45 wet=1.0")
            .unwrap_err();

        assert!(error.contains("delay time"));
    }

    #[test]
    fn bus_fx_command_rejects_invalid_feedback() {
        let mut session = ReplSession::new();
        session.eval_line(":bus new dub").unwrap();

        let error = session
            .eval_line(":bus fx dub delay time=3/16 feedback=1.5 wet=1.0")
            .unwrap_err();

        assert!(error.contains("feedback"));
    }

    #[test]
    fn bus_fx_command_rejects_invalid_wet() {
        let mut session = ReplSession::new();
        session.eval_line(":bus new dub").unwrap();

        let error = session
            .eval_line(":bus fx dub delay time=3/16 feedback=0.45 wet=1.5")
            .unwrap_err();

        assert!(error.contains("wet"));
    }

    #[test]
    fn bus_fx_command_rejects_invalid_reverb_size() {
        let mut session = ReplSession::new();
        session.eval_line(":bus new verb").unwrap();

        let error = session
            .eval_line(":bus fx verb reverb size=1.5 damp=0.35 wet=1.0")
            .unwrap_err();

        assert!(error.contains("size"));
    }

    #[test]
    fn bus_fx_command_rejects_invalid_reverb_damp() {
        let mut session = ReplSession::new();
        session.eval_line(":bus new verb").unwrap();

        let error = session
            .eval_line(":bus fx verb reverb size=0.75 damp=-0.1 wet=1.0")
            .unwrap_err();

        assert!(error.contains("damp"));
    }

    #[test]
    fn bus_fx_command_rejects_invalid_reverb_wet() {
        let mut session = ReplSession::new();
        session.eval_line(":bus new verb").unwrap();

        let error = session
            .eval_line(":bus fx verb reverb size=0.75 damp=0.35 wet=1.5")
            .unwrap_err();

        assert!(error.contains("wet"));
    }

    #[test]
    fn track_bind_command_rejects_unknown_bindings() {
        let mut session = ReplSession::new();

        session.eval_line(":track new drums").unwrap();

        assert_eq!(
            session.eval_line(":track bind drums nope"),
            Err("no binding named `nope`".to_owned())
        );
    }

    #[test]
    fn track_send_command_rejects_unknown_buses() {
        let mut session = ReplSession::new();

        session.eval_line("groove = bd sn").unwrap();
        session.eval_line(":track new drums").unwrap();
        session.eval_line(":track bind drums groove").unwrap();

        assert_eq!(
            session.eval_line(":send drums verb 0.35"),
            Err("no bus named `verb`".to_owned())
        );
    }

    #[test]
    fn track_new_command_rejects_duplicate_track_names() {
        let mut session = ReplSession::new();

        session.eval_line(":track new drums").unwrap();

        assert_eq!(
            session.eval_line(":track new drums"),
            Err("track `drums` already exists".to_owned())
        );
    }

    #[test]
    fn bus_new_command_rejects_duplicate_bus_names() {
        let mut session = ReplSession::new();

        session.eval_line(":bus new verb").unwrap();

        assert_eq!(
            session.eval_line(":bus new verb"),
            Err("bus `verb` already exists".to_owned())
        );
    }

    #[test]
    fn track_level_and_mute_commands_shape_live_output() {
        let mut session = ReplSession::new();
        let directory = temp_directory("repl-track-level-mute");
        write_wav(directory.join("bd.wav"), &[0.8, 0.0, 0.0, 0.0]);

        session
            .eval_line(&format!(":samples {}", directory.display()))
            .unwrap();
        session.eval_line("groove = bd").unwrap();
        session.eval_line(":track new drums").unwrap();
        session.eval_line(":track bind drums groove").unwrap();
        session.eval_line(":track level drums 0.5").unwrap();
        let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
        let leveled = session.render_test_block_for_tui(4);
        let leveled_expected = 0.4 * edge_envelope(0, 4);
        assert!((leveled[0] - leveled_expected).abs() < f32::EPSILON);

        session.eval_line(":track mute drums on").unwrap();
        let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
        let muted = session.render_test_block_for_tui(4);
        assert!(muted.iter().all(|sample| sample.abs() < f32::EPSILON));

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn track_rebinding_replaces_prior_assignment_without_haunted_output() {
        let mut session = ReplSession::new();
        let directory = temp_directory("repl-track-rebind");
        write_wav(directory.join("bd.wav"), &[0.1, 0.0, 0.0, 0.0]);
        write_wav(directory.join("sn.wav"), &[0.9, 0.0, 0.0, 0.0]);

        session
            .eval_line(&format!(":samples {}", directory.display()))
            .unwrap();
        session.eval_line("groove = bd").unwrap();
        session.eval_line("backbeat = sn").unwrap();
        session.eval_line(":track new drums").unwrap();
        session.eval_line(":track bind drums groove").unwrap();
        let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
        let first = session.render_test_block_for_tui(4);
        let first_expected = 0.1 * edge_envelope(0, 4);
        assert!((first[0] - first_expected).abs() < f32::EPSILON);

        session.eval_line(":track bind drums backbeat").unwrap();
        let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
        let second = session.render_test_block_for_tui(4);
        let second_expected = 0.9 * edge_envelope(0, 4);
        assert!((second[0] - second_expected).abs() < f32::EPSILON);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn roll_command_prints_ascii_roll() {
        let mut session = ReplSession::new();
        session.eval_line("pattern = bd sn").unwrap();

        let message = session.eval_line(":roll pattern 1 8").unwrap();

        assert!(message.contains("┌────────────────────────────────────┐"));
        assert!(message.contains("│ Pattern Roll: pattern (1 cycles)   │"));
        assert!(message.contains("╞════════════════════════════════════╡"));
        assert!(message.contains("│ bd │ x---....                      │"));
        assert!(message.contains("│ sn │ ....x---                      │"));
        assert!(message.contains("└────────────────────────────────────┘"));
    }

    #[test]
    fn roll_command_rejects_unknown_bindings() {
        let mut session = ReplSession::new();

        let error = session.eval_line(":roll nope").unwrap_err();

        assert!(error.contains("no binding named `nope`"));
    }

    #[test]
    fn roll_command_rejects_invalid_cycles() {
        let mut session = ReplSession::new();
        session.eval_line("pattern = bd sn").unwrap();

        let error = session.eval_line(":roll pattern foo").unwrap_err();

        assert!(error.contains("cycles must be a positive integer"));
    }

    #[test]
    fn stats_command_returns_sample_pattern_stats() {
        let mut session = ReplSession::new();
        session.eval_line("pattern = fast(2, bd sn)").unwrap();

        let message = session.eval_line(":stats pattern 2").unwrap();

        assert!(message.contains("Pattern Stats: pattern (2 cycles)"));
        assert!(message.contains("Total Events"));
        assert!(message.contains("8"));
        assert!(message.contains("Unique Samples"));
        assert!(message.contains("2 (bd, sn)"));
        assert!(message.contains("Event Density"));
        assert!(message.contains("4.00 events/cycle"));
    }

    #[test]
    fn stats_command_rejects_unknown_bindings() {
        let mut session = ReplSession::new();

        let error = session.eval_line(":stats nope").unwrap_err();

        assert!(error.contains("no binding named `nope`"));
    }

    #[test]
    fn stats_command_rejects_invalid_cycles() {
        let mut session = ReplSession::new();
        session.eval_line("pattern = bd sn").unwrap();

        let error = session.eval_line(":stats pattern foo").unwrap_err();

        assert!(error.contains("cycles must be a positive integer"));
    }

    #[test]
    fn render_command_exports_a_bound_pattern() {
        let mut session = ReplSession::new();
        let path = temp_wav_path();

        session.eval_line("song = bd sn cp sn").unwrap();
        let message = session
            .eval_line(&format!(":render song {} 2", path.display()))
            .unwrap();

        assert!(message.contains("rendered `song`"));
        assert!(path.exists());
        assert!(fs::metadata(&path).unwrap().len() > 44);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_command_exports_a_bound_pattern_to_csv() {
        let mut session = ReplSession::new();
        let path = temp_csv_path();

        session.eval_line("song = bd sn cp sn").unwrap();
        let message = session
            .eval_line(&format!(":export song {} 2", path.display()))
            .unwrap();

        assert!(message.contains("exported `song`"));
        assert!(path.exists());
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains(
            "start_num,start_den,start_float,end_num,end_den,end_float,sample,gain,pan,rate"
        ));
        assert!(contents.contains("bd"));
        assert!(contents.contains("sn"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_command_exports_a_bound_pattern_to_svg() {
        let mut session = ReplSession::new();
        let path = temp_svg_path();

        session.eval_line("song = bd sn cp sn").unwrap();
        let message = session
            .eval_line(&format!(":export song {} 2", path.display()))
            .unwrap();

        assert!(message.contains("exported `song`"));
        assert!(path.exists());
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(contents.contains("bd"));
        assert!(contents.contains("sn"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_command_exports_a_bound_pattern_to_json() {
        let mut session = ReplSession::new();
        let path = temp_json_path();

        session.eval_line("song = bd sn cp sn").unwrap();
        let message = session
            .eval_line(&format!(":export song {} 2", path.display()))
            .unwrap();

        assert!(message.contains("exported `song`"));
        assert!(path.exists());
        let contents = fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(json["kind"], "sample");
        assert_eq!(json["cycle_count"], 2);
        assert!(json["events"].is_array());
        assert_eq!(json["events"][0]["sample"], "bd");
        assert_eq!(json["events"][1]["sample"], "sn");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_command_exports_number_pattern_to_svg() {
        let mut session = ReplSession::new();
        let path = temp_svg_path();

        session.eval_line("notes = 1 2 3").unwrap();
        let message = session
            .eval_line(&format!(":export notes {} 1", path.display()))
            .unwrap();

        assert!(message.contains("exported `notes`"));
        assert!(path.exists());
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(contents.contains("<rect"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_command_exports_number_pattern_to_csv() {
        let mut session = ReplSession::new();
        let path = temp_csv_path();

        session.eval_line("notes = 1 2 3").unwrap();
        let message = session
            .eval_line(&format!(":export notes {} 1", path.display()))
            .unwrap();

        assert!(message.contains("exported `notes`"));
        assert!(path.exists());
        let contents = fs::read_to_string(&path).unwrap();
        assert!(
            contents.contains("start_num,start_den,start_float,end_num,end_den,end_float,value")
        );
        assert!(contents.contains('1'));
        assert!(contents.contains('2'));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_command_exports_number_pattern_to_json() {
        let mut session = ReplSession::new();
        let path = temp_json_path();

        session.eval_line("notes = 1 2 3").unwrap();
        let message = session
            .eval_line(&format!(":export notes {} 1", path.display()))
            .unwrap();

        assert!(message.contains("exported `notes`"));
        assert!(path.exists());
        let contents = fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(json["kind"], "number");
        assert_eq!(json["cycle_count"], 1);
        assert!(json["events"].is_array());
        assert_eq!(json["events"][0]["value"], 1.0);
        assert_eq!(json["events"][1]["value"], 2.0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_stems_command_writes_track_stems() {
        let mut session = ReplSession::new();

        session.eval_line("drums = bd").unwrap();
        session.eval_line("bass = cp").unwrap();
        session.eval_line(":track new drums_track").unwrap();
        session.eval_line(":track bind drums_track drums").unwrap();
        session.eval_line(":track new bass_track").unwrap();
        session.eval_line(":track bind bass_track bass").unwrap();

        let message = session.eval_line(":export stems 1").unwrap();
        assert!(message.contains("exported 2 stem(s)"));
        let rendered_dir = parse_exported_stem_dir(&message);
        assert!(rendered_dir.join("drums_track.wav").exists());
        assert!(rendered_dir.join("bass_track.wav").exists());

        fs::remove_dir_all(rendered_dir).unwrap();
    }

    #[test]
    fn export_stems_command_can_include_bus_stems() {
        let mut session = ReplSession::new();

        session.eval_line("drums = bd").unwrap();
        session.eval_line(":track new drums").unwrap();
        session.eval_line(":track bind drums drums").unwrap();
        session.eval_line(":bus new verb").unwrap();
        session
            .eval_line(":bus fx verb reverb size=0.75 damp=0.35 wet=1.0")
            .unwrap();
        session.eval_line(":send drums verb 1.0").unwrap();

        let message = session.eval_line(":export stems 1 --buses").unwrap();
        assert!(message.contains("exported 2 stem(s)"));
        let rendered_dir = parse_exported_stem_dir(&message);
        assert!(rendered_dir.join("drums.wav").exists());
        assert!(rendered_dir.join("verb_bus.wav").exists());

        fs::remove_dir_all(rendered_dir).unwrap();
    }

    #[test]
    fn samples_command_loads_directory_overrides_for_live_playback() {
        let mut session = ReplSession::new();
        let directory = temp_directory("repl-samples");
        write_wav(directory.join("bd.wav"), &[0.25, 0.0, 0.0, 0.0]);

        let message = session
            .eval_line(&format!(":samples {}", directory.display()))
            .unwrap();
        session.eval_line("drums = bd").unwrap();
        let rendered = session.render_test_block_for_tui(4);
        let expected = 0.25 * edge_envelope(0, 4);

        assert!(message.contains("loaded"));
        assert!((rendered[0] - expected).abs() < f32::EPSILON);
        assert!((rendered[1] - expected).abs() < f32::EPSILON);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reload_samples_command_swaps_sample_bank_at_cycle_boundary() {
        let mut session = ReplSession::new();
        let directory = temp_directory("repl-reload");
        write_wav(directory.join("bd.wav"), &[0.1, 0.0, 0.0, 0.0]);

        session
            .eval_line(&format!(":samples {}", directory.display()))
            .unwrap();
        session.eval_line("drums = bd bd").unwrap();

        let first_trigger = session.render_test_block_for_tui(4);
        let first_expected = 0.1 * edge_envelope(0, 4);
        assert!((first_trigger[0] - first_expected).abs() < f32::EPSILON);

        write_wav(directory.join("bd.wav"), &[0.9, 0.0, 0.0, 0.0]);
        let message = session.eval_line(":reload-samples").unwrap();
        assert!(message.contains("reloaded"));

        let frames_per_cycle = session.transport_snapshot().frames_per_cycle();
        let frames_until_second_trigger = (frames_per_cycle / 2).saturating_sub(4);
        let _ = session.render_test_block_for_tui(frames_until_second_trigger);
        let second_trigger_same_cycle = session.render_test_block_for_tui(4);
        assert!((second_trigger_same_cycle[0] - first_expected).abs() < f32::EPSILON);

        let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
        let first_trigger_next_cycle = session.render_test_block_for_tui(4);
        let reloaded_expected = 0.9 * edge_envelope(0, 4);
        assert!((first_trigger_next_cycle[0] - reloaded_expected).abs() < f32::EPSILON);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn sample_playback_params_flow_into_live_engine() {
        let mut session = ReplSession::new();
        let directory = temp_directory("repl-sample-params");
        fs::write(
            directory.join("samples.ron"),
            "(\n  tokens: {\n    \"vox_ah\": \"vox.wav\",\n  },\n)\n",
        )
        .unwrap();
        write_wav(directory.join("vox.wav"), &[0.2, 0.4, 0.6, 0.8]);

        session
            .eval_line(&format!(":samples {}", directory.display()))
            .unwrap();
        session
            .eval_line(
                r#"lead = sample("vox_ah") |> slice(0.25, 1) |> rate(2) |> gain(0.5) |> pan(-1)"#,
            )
            .unwrap();

        let rendered = session.render_test_block_for_tui(4);
        let first_expected = 0.2 * edge_envelope(0, 2);
        let second_expected = 0.4 * edge_envelope(1, 2);
        assert!((rendered[0] - first_expected).abs() < f32::EPSILON);
        assert!(rendered[1].abs() < f32::EPSILON);
        assert!((rendered[2] - second_expected).abs() < f32::EPSILON);
        assert!(rendered[3].abs() < f32::EPSILON);

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn synth_pulse_width_controls_flow_into_live_engine() {
        let mut narrow = ReplSession::new();
        narrow.eval_line("lead = pulse |> pw(0.25)").unwrap();
        let narrow_rendered = narrow.render_test_block_for_tui(64);

        let mut wide = ReplSession::new();
        wide.eval_line("lead = pulse |> pw(0.75)").unwrap();
        let wide_rendered = wide.render_test_block_for_tui(64);

        assert!(narrow_rendered.iter().all(|sample| sample.is_finite()));
        assert!(wide_rendered.iter().all(|sample| sample.is_finite()));
        assert!(
            narrow_rendered
                .iter()
                .zip(&wide_rendered)
                .any(|(left, right)| (left - right).abs() > f32::EPSILON)
        );
    }

    #[test]
    fn open_command_replaces_session_bindings_from_ode_file() {
        let mut session = ReplSession::new();
        let song = fixture("song.ode");

        session.eval_line("scratch = 1").unwrap();

        let message = session
            .eval_line(&format!(":open {}", song.display()))
            .unwrap();

        assert!(message.contains("opened"));
        assert_eq!(
            session.binding_summaries(),
            vec![
                "drums: Pattern<Sample>".to_owned(),
                "song: Pattern<Sample>".to_owned()
            ]
        );
        assert_eq!(session.last_loaded_pattern_name(), Some("song".to_owned()));
        assert_eq!(
            session.eval_line("copy = song"),
            Ok("bound copy: Pattern<Sample>".to_owned())
        );
        assert_eq!(
            session.eval_line(":render scratch out.wav 1"),
            Err("no binding named `scratch`".to_owned())
        );
    }

    #[test]
    fn render_command_rejects_unknown_bindings() {
        let mut session = ReplSession::new();

        let error = session.eval_line(":render nope out.wav 1").unwrap_err();

        assert!(error.contains("no binding named `nope`"));
    }

    #[test]
    fn tempo_command_updates_engine_transport() {
        let mut session = ReplSession::new();

        let message = session.eval_line(":tempo 90").unwrap();
        let _ = session.render_test_block_for_tui(1);

        assert_eq!(message, "tempo set to 90 BPM");
        assert_eq!(
            session.transport_snapshot().tempo_bpm().to_bits(),
            90.0_f32.to_bits()
        );
    }

    #[test]
    fn tempo_command_rejects_non_positive_values() {
        let mut session = ReplSession::new();

        let error = session.eval_line(":tempo 0").unwrap_err();

        assert_eq!(error, "tempo must be a finite positive BPM");
    }

    #[test]
    fn stop_and_play_commands_update_transport_state() {
        let mut session = ReplSession::new();
        session.eval_line("drums = bd sn cp sn").unwrap();
        let _ = session.render_test_block_for_tui(256);

        let stop_message = session.eval_line(":stop").unwrap();
        let _ = session.render_test_block_for_tui(1);
        assert_eq!(stop_message, "transport stopped");
        assert!(!session.transport_snapshot().is_playing());

        let play_message = session.eval_line(":play").unwrap();
        let _ = session.render_test_block_for_tui(1);
        assert_eq!(play_message, "transport playing");
        assert!(session.transport_snapshot().is_playing());
    }

    #[test]
    fn midi_list_command_returns_available_outputs_or_none() {
        let mut session = ReplSession::new();
        let message = session.eval_line(":midi list").unwrap();
        assert!(message.starts_with("available MIDI output ports: "));
    }

    #[test]
    fn midi_input_list_command_returns_available_inputs_or_none() {
        let mut session = ReplSession::new();
        let message = session.eval_line(":midi in list").unwrap();
        assert!(message.starts_with("available MIDI input ports: "));
    }

    #[test]
    fn midi_send_command_requires_active_connection() {
        let mut session = ReplSession::new();
        session.eval_line("notes = 60 64 67").unwrap();

        let error = session.eval_line(":midi send notes 1").unwrap_err();
        assert!(error.contains("no MIDI output is connected"));
    }

    #[test]
    fn midi_send_command_rejects_non_number_patterns() {
        let mut session = ReplSession::new();
        session.eval_line("drums = bd sn").unwrap();

        let error = session.eval_line(":midi send drums 1").unwrap_err();
        assert!(error.contains("cannot be sent as MIDI notes"));
    }

    #[test]
    fn midi_cc_builtin_reads_normalized_controller_value() {
        let mut session = ReplSession::new();
        crate::midi_input::set_cc_value_for_test(1, 64);
        session.eval_line("control = cc(1)").unwrap();
        let crate::Value::NumberPattern(pattern) = session.bindings.get("control").unwrap() else {
            panic!("expected number pattern");
        };
        let value = pattern.constant_value().unwrap();
        assert!((value - (64.0 / 127.0)).abs() < 1e-9);
    }

    #[test]
    fn last_loaded_pattern_name_tracks_sample_bindings() {
        let mut session = ReplSession::new();

        session.eval_line("drums = bd sn cp sn").unwrap();
        assert_eq!(session.last_loaded_pattern_name(), Some("drums".to_owned()));

        session.eval_line("warp = fast(2)").unwrap();
        assert_eq!(session.last_loaded_pattern_name(), Some("drums".to_owned()));
    }

    #[test]
    fn transport_view_tracks_active_and_pending_pattern_names() {
        let mut session = ReplSession::new();

        session.eval_line("drums = bd sn").unwrap();
        let view = session.transport_view();
        assert_eq!(view.active_pattern_name(), None);
        assert_eq!(view.pending_pattern_name(), Some("drums"));

        let _ = session.render_test_block_for_tui(256);
        let view = session.transport_view();
        assert_eq!(view.active_pattern_name(), Some("drums"));
        assert_eq!(view.pending_pattern_name(), None);

        session.eval_line("backbeat = sn cp").unwrap();
        let view = session.transport_view();
        assert_eq!(view.active_pattern_name(), Some("drums"));
        assert_eq!(view.pending_pattern_name(), Some("backbeat"));

        let _ = session.render_test_block_for_tui(1);
        let view = session.transport_view();
        assert_eq!(view.active_pattern_name(), Some("drums"));
        assert_eq!(view.pending_pattern_name(), Some("backbeat"));

        let _ = session.render_test_block_for_tui(session.engine.frames_until_boundary_for_test());
        let view = session.transport_view();
        assert_eq!(view.active_pattern_name(), Some("backbeat"));
        assert_eq!(view.pending_pattern_name(), None);
    }

    fn temp_directory(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("orpheus-samples-{name}-{}", unique_temp_suffix()));
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn unique_temp_suffix() -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        format!("{timestamp}-{counter}")
    }

    fn parse_exported_stem_dir(message: &str) -> PathBuf {
        let start = message
            .find('`')
            .unwrap_or_else(|| panic!("expected export path in message: {message}"));
        let end = message[start + 1..]
            .find('`')
            .map(|index| start + 1 + index)
            .unwrap_or_else(|| panic!("expected export path in message: {message}"));
        PathBuf::from(&message[start + 1..end])
    }

    fn write_wav(path: impl AsRef<Path>, frames: &[f32]) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec).unwrap();
        for sample in frames {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
    }

    fn edge_envelope(frame_index: u32, total_frames: u32) -> f32 {
        let ramp_frames = total_frames.div_ceil(2).clamp(1, 32);
        let attack = normalized_edge_gain(frame_index, ramp_frames);
        let release = normalized_edge_gain(
            total_frames.saturating_sub(frame_index.saturating_add(1)),
            ramp_frames,
        );
        attack.min(release)
    }

    #[allow(clippy::cast_precision_loss)]
    fn normalized_edge_gain(distance_from_edge: u32, ramp_frames: u32) -> f32 {
        (((distance_from_edge as f32) + 0.5) / (ramp_frames as f32)).min(1.0)
    }
}
