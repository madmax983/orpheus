#![allow(clippy::map_unwrap_or, clippy::single_char_pattern)]
//! The `session` module manages the interactive state of an Orpheus environment.
//!
//! This module forms the bridge between the textual inputs of the user (via the REPL or TUI)
//! and the executing backend, managing the bindings of variables, the loading of external
//! sample banks, and real-time DSP commands (like tempo changes or transport control).
//!
//! The central type is `ReplSession`, which maintains a `BTreeMap` of variable names to
//! typed Orpheus [`Value`]s and interfaces directly with the `orpheus_dsp` layer via an
//! `EngineHandle`.

use crate::explain::Explain;
use ratatui::text::Line;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use midir::{Ignore, MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};
use orpheus_dsp::{
    DEFAULT_ANALOG_BASE_FREQUENCY_HZ, EngineCommand, EngineHandle, GeneratorCycle,
    GeneratorCycleSpec, GeneratorId, GraphVoiceBank, GraphVoiceSpec, LevelSnapshot, PatternUpdate,
    SampleBank, SampleLibraryWatcher, SampleLibraryWatcherConfig, SampleTrigger, TransportSnapshot,
    load_sample_bank_from_directory, render_routing_snapshot_to_master_wav,
    render_routing_snapshot_to_stem_wavs,
};
use orpheus_pattern::{Event, Rational};

use crate::eval::eval_into_bindings_with_samples;
use crate::export::render_sample_pattern_to_file_with_bank;
use crate::export::sample_trigger_from_event;
use crate::loader::load_file_runtime_strict;
use crate::midi_input;
use crate::mixer::MixerState;
use crate::types::infer_into_bindings;
use crate::value::{SampleEvent, SamplePatternValue};
use crate::{ReplMode, Type, Value};

const SESSION_HISTORY_LIMIT: usize = 50;

/// Upper bound on the number of delivered generator cycle buffers retained per
/// slot for offline stem export (ADR 0009 addendum). Buffers are tiny (one
/// grid cycle of events), but a long-running grid would otherwise grow the
/// record without bound; once the cap is reached, later cycles are not
/// recorded and an export past this length loops the last retained cycle,
/// matching the engine's starvation-loops-last-buffer behavior.
const MAX_RECORDED_GENERATOR_CYCLES: usize = 4096;

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
    sample_bank: Arc<SampleBank>,
    sample_directory: Option<PathBuf>,
    sample_watcher: Option<SampleLibraryWatcher>,
    bindings: BTreeMap<String, Value>,
    type_bindings: BTreeMap<String, Type>,
    mixer: MixerState,
    /// Per-slot record of the generator cycle buffers delivered to the engine
    /// since the source last started, keyed by [`GeneratorId`] (ADR 0009
    /// addendum). Offline stem export replays these buffers so generator
    /// tracks (e.g. the Orca grid) are audible in exported stems, exactly as
    /// they were delivered from grid start. Purely off-thread bookkeeping — the
    /// audio path never reads it.
    generator_cycles: BTreeMap<GeneratorId, Vec<Box<[Event<SampleTrigger>]>>>,
    /// Live multi-cycle arrangement drivers, keyed by the synthetic
    /// [`GeneratorId`] the live routing snapshot assigned to each plain
    /// sample-pattern track (issue #1446). The value is the binding name to
    /// query at each engine cycle boundary; the queried cycle is pushed over
    /// the generator ring so the arrangement advances section by section
    /// instead of looping cycle 0. Populated by [`Self::enqueue_mixer_snapshot`]
    /// and drained by [`Self::poll_arrangements`], both on the control thread.
    arrangement_generators: BTreeMap<GeneratorId, String>,
    /// The engine cycle-start frame the arrangement drivers last delivered a
    /// cycle for, so [`Self::poll_arrangements`] only queries and pushes once
    /// per boundary — the same latch the Orca publisher uses.
    arrangement_last_cycle_start: Option<u64>,
    /// The most recently delivered cycle buffer per arrangement generator, kept
    /// only so tests can observe that consecutive cycles differ. Off-thread
    /// bookkeeping; the audio path never reads it.
    last_arrangement_cycles: BTreeMap<GeneratorId, Vec<Event<SampleTrigger>>>,
    pattern_display: RefCell<PatternDisplayState>,
    midi_output: MidiOutputState,
    midi_input: MidiInputState,
    midi_note_mappings: HashMap<u8, String>,
    tempo_bpm: f32,
    reference_frequency_hz: f32,
    history: SessionHistory,
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

#[derive(Clone)]
struct SessionSnapshot {
    sample_bank: Arc<SampleBank>,
    sample_directory: Option<PathBuf>,
    bindings: BTreeMap<String, Value>,
    type_bindings: BTreeMap<String, Type>,
    mixer: MixerState,
    pattern_display: PatternDisplayState,
    tempo_bpm: f32,
    reference_frequency_hz: f32,
}

struct SessionHistory {
    undo_stack: VecDeque<SessionSnapshot>,
    redo_stack: Vec<SessionSnapshot>,
    limit: usize,
}

impl Default for SessionHistory {
    fn default() -> Self {
        Self::with_limit(SESSION_HISTORY_LIMIT)
    }
}

impl SessionHistory {
    const fn with_limit(limit: usize) -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: Vec::new(),
            limit,
        }
    }

    fn record(&mut self, snapshot: SessionSnapshot) {
        if self.limit == 0 {
            return;
        }
        self.push_undo(snapshot);
        self.redo_stack.clear();
    }

    fn undo(&mut self, current: SessionSnapshot) -> Option<SessionSnapshot> {
        let target = self.undo_stack.pop_back()?;
        self.redo_stack.push(current);
        Some(target)
    }

    fn redo(&mut self, current: SessionSnapshot) -> Option<SessionSnapshot> {
        let target = self.redo_stack.pop()?;
        self.push_undo(current);
        Some(target)
    }

    fn push_undo(&mut self, snapshot: SessionSnapshot) {
        if self.limit == 0 {
            return;
        }
        if self.undo_stack.len() == self.limit {
            let _ = self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(snapshot);
    }
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
/// assert!(view.has_pending_routing());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixerView {
    has_pending_routing: bool,
    summary: String,
    tui_summary: Vec<Line<'static>>,
}

impl TransportView {
    /// Exposes a reference to the underlying DSP transport snapshot.
    ///
    /// This state is snapshotted from the audio thread and is safe for the
    /// REPL UI to read without acquiring locks, allowing it to render the
    /// current playhead position without interrupting audio generation.
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

    /// Retrieves the string name of the currently active (playing) pattern, if any.
    ///
    /// This provides visual feedback to the user about which binding is currently
    /// driving the audio engine, allowing them to verify that their intended
    /// code is live.
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

    /// The string name of the pattern pending execution at the next cycle boundary.
    ///
    /// Live-coding is inherently asynchronous: a user might execute a new pattern
    /// (e.g. `drums = bd sn fast(2, cp)`) while the current measure is only halfway finished.
    /// The TUI needs this method to visually indicate to the user which pattern is "cued up"
    /// and waiting for the next downbeat to take over.
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
    /// Indicates whether there are uncommitted routing changes waiting to be applied at the next cycle boundary.
    ///
    /// Live-coding is inherently asynchronous. When a user executes a mixer command
    /// (e.g. `:track new drums`), the change doesn't happen instantly; it is scheduled for the next
    /// downbeat. This flag allows the TUI to visually highlight pending mixer topologies.
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

    /// Retrieves a pre-formatted, human-readable summary of the active mixer routing graph.
    ///
    /// Constructing strings and formatting graphs is expensive and shouldn't block the TUI
    /// render thread. Therefore, the `Session` caches this layout string whenever the topology
    /// changes, allowing the TUI to quickly paint the current routing state to the terminal.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// Exposes the pre-rendered, colorized UI lines of the session's active bindings and graph.
    ///
    /// While [`MixerView::summary`] exposes a raw text representation, `tui_summary` provides the fully
    /// styled and laid-out equivalent for direct rendering in a `ratatui` interface. This avoids
    /// re-parsing and styling the text on every draw tick, keeping the UI thread responsive.
    ///
    /// ## Examples
    ///
    /// ```
    /// use ratatui::text::Line;
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// session.eval_line("notes = 60 64 67");
    ///
    /// // The lines are pre-rendered and styled with ANSI colors
    /// let mixer = session.mixer_view();
    /// let lines: &[Line<'static>] = mixer.tui_summary();
    /// assert!(!lines.is_empty());
    /// ```
    #[must_use]
    pub fn tui_summary(&self) -> &[Line<'static>] {
        &self.tui_summary
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
        let tempo_bpm = engine.transport_snapshot().tempo_bpm();
        Self {
            mode: ReplMode::Loose,
            engine,
            sample_bank: Arc::new(SampleBank::load_builtin()),
            sample_directory: None,
            sample_watcher: None,
            bindings: BTreeMap::new(),
            type_bindings: BTreeMap::new(),
            mixer: MixerState::default(),
            generator_cycles: BTreeMap::new(),
            arrangement_generators: BTreeMap::new(),
            arrangement_last_cycle_start: None,
            last_arrangement_cycles: BTreeMap::new(),
            pattern_display: RefCell::new(PatternDisplayState::default()),
            midi_output: MidiOutputState::default(),
            midi_input: MidiInputState::default(),
            midi_note_mappings: HashMap::new(),
            tempo_bpm,
            reference_frequency_hz: DEFAULT_ANALOG_BASE_FREQUENCY_HZ,
            history: SessionHistory::default(),
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
    /// assert_eq!(response, "bound notes = Pattern<Number>: Pattern<Number>");
    ///
    /// // Execute a command.
    /// let response = session.eval_line(":tempo 120").unwrap();
    /// assert_eq!(response, "tempo set to 120 BPM");
    /// ```
    pub fn eval_line(&mut self, source: &str) -> Result<String, String> {
        self.apply_midi_note_mappings()?;
        self.poll_sample_watcher()?;
        if source.starts_with(':') {
            return self.eval_command(source);
        }

        let snapshot = self.capture_history_snapshot();
        let Some((name, ty)) = infer_into_bindings(source, self.mode, &mut self.type_bindings)
            .map_err(|error| error.to_string())?
        else {
            return Err("no bindings were produced".into());
        };
        let Some((value_name, value)) = eval_into_bindings_with_samples(
            source,
            self.mode,
            &mut self.bindings,
            Arc::clone(&self.sample_bank),
        )
        .map_err(|error| error.to_string())?
        else {
            return Err("no bindings were produced".into());
        };
        debug_assert_eq!(name, value_name);

        self.push_pattern_update(&name, &value)?;
        if matches!(value, Value::Voice(_)) {
            self.sync_graph_voice_programs()?;
        }
        self.history.record(snapshot);
        Ok(success_banner(&name, &value, &ty))
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
            "undo" => self.undo_command(args),
            "redo" => self.redo_command(args),
            "render" => self.render_binding(args),
            "roll" => self.roll_binding(args),
            "stats" => self.stats_binding(args),
            "explain" => self.explain_binding(args),
            "help" => Self::help_command(args),
            "env" => self.env_command(args),
            "export" => {
                if args.starts_with("stems") {
                    self.export_stems(args)
                } else if args.starts_with("master") {
                    self.export_master(args)
                } else {
                    self.export_binding(args)
                }
            }
            "tempo" => self.eval_mutating_command(|session| session.set_tempo(args)),
            "ref_freq" => self.eval_mutating_command(|session| session.set_ref_freq(args)),
            "samples" => self.eval_mutating_command(|session| session.load_sample_directory(args)),
            "import" => self.eval_mutating_command(|session| session.import_command(args)),
            "open" => self.eval_mutating_command(|session| session.open_file(args)),
            "track" => self.eval_mutating_command(|session| session.eval_track_command(args)),
            "bus" => self.eval_mutating_command(|session| session.eval_bus_command(args)),
            "send" => self.eval_mutating_command(|session| session.eval_send_command(args)),
            "mixer" => self.mixer_command(args),
            "midi" => self.midi_command(args),
            "reload-samples" => {
                self.eval_mutating_command(|session| session.reload_sample_directory(args))
            }
            "play" => self.play_transport(args),
            "stop" => self.stop_transport(args),
            other => Err(format!("unknown REPL command `:{other}`")),
        }
    }

    fn eval_mutating_command(
        &mut self,
        run: impl FnOnce(&mut Self) -> Result<String, String>,
    ) -> Result<String, String> {
        let snapshot = self.capture_history_snapshot();
        match run(self) {
            Ok(message) => {
                self.history.record(snapshot);
                Ok(message)
            }
            Err(error) => Err(error),
        }
    }

    fn undo_command(&mut self, args: &str) -> Result<String, String> {
        if !args.trim().is_empty() {
            return Err(undo_usage().to_owned());
        }

        let current = self.capture_history_snapshot();
        let snapshot = self
            .history
            .undo(current)
            .ok_or_else(|| "nothing to undo".to_owned())?;
        self.restore_history_snapshot(snapshot)?;
        Ok("undid last session change".to_owned())
    }

    fn redo_command(&mut self, args: &str) -> Result<String, String> {
        if !args.trim().is_empty() {
            return Err(redo_usage().to_owned());
        }

        let current = self.capture_history_snapshot();
        let snapshot = self
            .history
            .redo(current)
            .ok_or_else(|| "nothing to redo".to_owned())?;
        self.restore_history_snapshot(snapshot)?;
        Ok("redid session change".to_owned())
    }

    fn capture_history_snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            sample_bank: self.sample_bank.clone(),
            sample_directory: self.sample_directory.clone(),
            bindings: self.bindings.clone(),
            type_bindings: self.type_bindings.clone(),
            mixer: self.mixer.clone(),
            pattern_display: self.pattern_display.borrow().clone(),
            tempo_bpm: self.tempo_bpm,
            reference_frequency_hz: self.reference_frequency_hz,
        }
    }

    fn restore_history_snapshot(&mut self, snapshot: SessionSnapshot) -> Result<(), String> {
        let routing_snapshot = snapshot.mixer.compile_snapshot(&snapshot.bindings)?;
        let had_voice_bindings = self.has_voice_bindings();

        if self.sample_bank != snapshot.sample_bank {
            self.engine
                .enqueue(EngineCommand::ReplaceSampleBank(
                    snapshot.sample_bank.clone(),
                ))
                .map_err(|error| format!("failed to enqueue restored sample bank: {error}"))?;
        }
        if self.tempo_bpm.to_bits() != snapshot.tempo_bpm.to_bits() {
            self.engine
                .enqueue(EngineCommand::SetTempo(snapshot.tempo_bpm))
                .map_err(|error| format!("failed to enqueue restored tempo: {error}"))?;
        }
        if self.reference_frequency_hz.to_bits() != snapshot.reference_frequency_hz.to_bits() {
            self.engine
                .enqueue(EngineCommand::SetReferenceFrequency(
                    snapshot.reference_frequency_hz,
                ))
                .map_err(|error| {
                    format!("failed to enqueue restored reference frequency: {error}")
                })?;
        }
        self.engine
            .enqueue(EngineCommand::SwapRoutingSnapshot(routing_snapshot))
            .map_err(|error| format!("failed to enqueue restored routing snapshot: {error}"))?;

        // History restore compiles a static snapshot; drop any live arrangement
        // drivers so they do not keep pushing into a routing that no longer
        // contains their generator slots (issue #1446).
        self.arrangement_generators.clear();
        self.arrangement_last_cycle_start = None;
        self.last_arrangement_cycles.clear();

        self.sample_bank = snapshot.sample_bank;
        self.sample_directory = snapshot.sample_directory;
        self.bindings = snapshot.bindings;
        self.type_bindings = snapshot.type_bindings;
        self.mixer = snapshot.mixer;
        *self.pattern_display.borrow_mut() = snapshot.pattern_display;
        self.tempo_bpm = snapshot.tempo_bpm;
        self.reference_frequency_hz = snapshot.reference_frequency_hz;
        if had_voice_bindings || self.has_voice_bindings() {
            self.sync_graph_voice_programs()?;
        }
        self.restart_sample_watcher_after_restore()
    }

    fn restart_sample_watcher_after_restore(&mut self) -> Result<(), String> {
        self.sample_watcher = None;
        if let Some(directory) = self.sample_directory.clone() {
            self.start_sample_watcher(&directory)?;
        }
        Ok(())
    }

    fn render_binding(&self, args: &str) -> Result<String, String> {
        let tokens: Vec<_> = args.split_whitespace().collect();
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

        render_sample_pattern_to_file_with_bank(pattern, &path, cycles, Arc::clone(&self.sample_bank))
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
                crate::value::Value::Tuning(tuning) => {
                    let stats = crate::stats::tuning_stats(binding_name, tuning);
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

    fn help_command(args: &str) -> Result<String, String> {
        if !args.trim().is_empty() {
            return Err("usage: :help (no arguments)".to_owned());
        }
        let table = build_help_table();
        Ok(format!(
            "\n\x1b[38;5;14m\x1b[1mREPL Commands:\x1b[0m\n{table}"
        ))
    }

    fn env_command(&self, args: &str) -> Result<String, String> {
        if !args.trim().is_empty() {
            return Err("usage: :env (no arguments)".to_owned());
        }

        let mut table = comfy_table::Table::new();
        table.load_preset(comfy_table::presets::UTF8_BORDERS_ONLY);
        table.set_header(vec![
            comfy_table::Cell::new("Binding")
                .fg(comfy_table::Color::White)
                .add_attribute(comfy_table::Attribute::Bold),
            comfy_table::Cell::new("Type")
                .fg(comfy_table::Color::White)
                .add_attribute(comfy_table::Attribute::Bold),
        ]);

        let summaries = self.binding_summaries();
        if summaries.is_empty() {
            return Ok("environment is empty".to_owned());
        }

        for summary in summaries {
            if let Some((name, ty)) = summary.split_once(": ") {
                table.add_row(vec![
                    comfy_table::Cell::new(name).fg(comfy_table::Color::Cyan),
                    comfy_table::Cell::new(ty)
                        .fg(comfy_table::Color::Yellow)
                        .set_alignment(comfy_table::CellAlignment::Right),
                ]);
            }
        }

        Ok(format!("\n{table}"))
    }

    fn explain_binding(&self, args: &str) -> Result<String, String> {
        let binding_name = args.trim();
        if binding_name.is_empty() || binding_name.contains(char::is_whitespace) {
            return Err(explain_usage().to_owned());
        }

        let value = self
            .bindings
            .get(binding_name)
            .ok_or_else(|| format!("no binding named `{binding_name}`"))?;

        match value {
            crate::value::Value::SamplePattern(pattern) => Ok(pattern.explain(binding_name)),
            crate::value::Value::NumberPattern(pattern) => Ok(pattern.explain(binding_name)),
            crate::value::Value::Function(func) => Ok(func.explain(binding_name)),
            crate::value::Value::Pedal(pedal) => Ok(pedal.explain(binding_name)),
            crate::value::Value::Voice(voice) => Ok(voice.explain(binding_name)),
            crate::value::Value::Tuning(tuning) => Ok(tuning.explain(binding_name)),
            _ => Err(format!(
                "binding `{binding_name}` is a {} and cannot be explained",
                value.kind_name()
            )),
        }
    }

    fn export_binding(&self, args: &str) -> Result<String, String> {
        let tokens: Vec<_> = args.split_whitespace().collect();
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
        let tokens: Vec<_> = args.split_whitespace().collect();
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
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();

        let export_dir = if cfg!(test) {
            std::env::temp_dir().join(format!("orpheus-stems-{timestamp}-{nanos}"))
        } else {
            PathBuf::from("exports").join(format!("stems-{timestamp}"))
        };

        let tempo_bpm = self.transport_snapshot().tempo_bpm();
        let graph_voice_specs = self.graph_voice_specs()?;
        let generator_cycles = self.generator_cycle_specs(cycles);
        let written = render_routing_snapshot_to_stem_wavs(
            &snapshot,
            cycles,
            tempo_bpm,
            &self.sample_bank,
            &graph_voice_specs,
            &generator_cycles,
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

    fn export_master(&self, args: &str) -> Result<String, String> {
        let tokens: Vec<_> = args.split_whitespace().collect();
        if tokens.first().copied() != Some("master") {
            return Err(master_export_usage().to_owned());
        }

        let mut cycles = 1_u64;
        // A master mixdown is the full front-of-house sum, so bus returns
        // (reverb/delay tails) are folded in by default; `--no-buses` renders a
        // dry, track-only master.
        let mut include_buses = true;
        let mut path: Option<String> = None;
        for token in tokens.iter().skip(1) {
            match *token {
                "--no-buses" => include_buses = false,
                "--buses" => include_buses = true,
                other => {
                    if let Ok(parsed) = other.parse::<u64>() {
                        cycles = parsed;
                    } else if path.is_none() {
                        path = Some(other.to_owned());
                    } else {
                        return Err(master_export_usage().to_owned());
                    }
                }
            }
        }

        let path = path.ok_or_else(|| master_export_usage().to_owned())?;
        if cycles == 0 {
            return Err("cycles must be a positive integer".to_owned());
        }

        // Materialize any finite multi-cycle arrangement (e.g. `seq_sections`)
        // per cycle so an offline master render advances section by section
        // instead of looping cycle 0 (issue #1446). Plain sample-pattern tracks
        // are compiled to synthetic per-cycle generators here; real generator
        // tracks keep their recorded buffers, appended below.
        let (snapshot, mut generator_cycles) = self
            .mixer
            .compile_offline_snapshot(&self.bindings, cycles)?;
        let has_active_tracks = snapshot
            .tracks()
            .iter()
            .any(|track| !track.source().is_unbound() && !track.muted());
        if !has_active_tracks {
            return Err(
                "no active tracks to export; bind a sample pattern or create/bind tracks first"
                    .to_owned(),
            );
        }

        let tempo_bpm = self.transport_snapshot().tempo_bpm();
        let graph_voice_specs = self.graph_voice_specs()?;
        generator_cycles.extend(self.generator_cycle_specs(cycles));
        let (written, stats) = render_routing_snapshot_to_master_wav(
            &snapshot,
            cycles,
            tempo_bpm,
            &self.sample_bank,
            &graph_voice_specs,
            &generator_cycles,
            &path,
            include_buses,
        )
        .map_err(|error| error.to_string())?;

        let clip_note = if stats.peak > 1.0 {
            format!(
                " (warning: master peak {:.2} exceeds full scale)",
                stats.peak
            )
        } else {
            String::new()
        };
        Ok(format!(
            "exported master to `{}` ({cycles} cycle(s)){clip_note}",
            written.display()
        ))
    }

    fn export_sample_pattern(
        pattern: &crate::value::SamplePatternValue,
        path: &str,
        cycles: u64,
    ) -> Result<(), String> {
        let export_path = std::path::Path::new(path);
        let ext = export_path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);

        match ext.as_deref() {
            Some("svg") => crate::svg::export_sample_pattern_to_svg(pattern, path, cycles),
            Some("html") => crate::html::export_sample_pattern_to_html(pattern, path, cycles),
            Some("json") => crate::export::export_sample_pattern_to_json(pattern, path, cycles),
            Some("md") => crate::export::export_sample_pattern_to_md(pattern, path, cycles),
            Some("srt") => crate::srt::export_sample_pattern_to_srt(pattern, path, cycles),
            Some("txt") => crate::txt::export_sample_pattern_to_txt(pattern, path, cycles),
            Some("trk" | "tracker") => {
                crate::tracker::export_sample_pattern_to_tracker(pattern, path, cycles)
            }
            Some("mid" | "midi") => {
                crate::midi_export::export_sample_pattern_to_midi(pattern, path, cycles)
            }
            Some("scd") => crate::supercollider_export::export_sample_pattern_to_supercollider(
                pattern, path, cycles,
            ),
            _ => crate::export::export_sample_pattern_to_csv(pattern, path, cycles),
        }
        .map_err(|error: crate::EvalError| error.to_string())?;

        Ok(())
    }

    fn export_number_pattern(
        pattern: &crate::value::NumberPatternValue,
        path: &str,
        cycles: u64,
    ) -> Result<(), String> {
        let export_path = std::path::Path::new(path);
        let ext = export_path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);

        match ext.as_deref() {
            Some("svg") => crate::svg::export_number_pattern_to_svg(pattern, path, cycles),
            Some("html") => crate::html::export_number_pattern_to_html(pattern, path, cycles),
            Some("json") => crate::export::export_number_pattern_to_json(pattern, path, cycles),
            Some("md") => crate::export::export_number_pattern_to_md(pattern, path, cycles),
            Some("srt") => crate::srt::export_number_pattern_to_srt(pattern, path, cycles),
            Some("txt") => crate::txt::export_number_pattern_to_txt(pattern, path, cycles),
            #[cfg(feature = "lilypond_export")]
            Some("ly") => {
                crate::lilypond_export::export_number_pattern_to_lilypond(pattern, path, cycles)
            }
            Some("trk" | "tracker") => {
                crate::tracker::export_number_pattern_to_tracker(pattern, path, cycles)
            }
            Some("mid" | "midi") => {
                crate::midi_export::export_number_pattern_to_midi(pattern, path, cycles)
            }
            Some("abc") => crate::abc_export::export_number_pattern_to_abc(pattern, path, cycles),
            Some("scd") => crate::supercollider_export::export_number_pattern_to_supercollider(
                pattern, path, cycles,
            ),
            _ => crate::export::export_number_pattern_to_csv(pattern, path, cycles),
        }
        .map_err(|error: crate::EvalError| error.to_string())?;

        Ok(())
    }

    fn export_pattern_value(
        value: &Value,
        path: &str,
        cycles: u64,
        binding_name: &str,
    ) -> Result<(), String> {
        match value {
            Value::SamplePattern(pattern) => Self::export_sample_pattern(pattern, path, cycles)?,
            Value::NumberPattern(pattern) => Self::export_number_pattern(pattern, path, cycles)?,
            Value::ArpDirection(_)
            | Value::PitchClassSet(_)
            | Value::Function(_)
            | Value::Pedal(_)
            | Value::Voice(_)
            | Value::PluginPattern(_)
            | Value::Tuning(_)
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
        let tokens: Vec<_> = args.split_whitespace().collect();
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
        self.tempo_bpm = tempo_bpm;
        Ok(format!("tempo set to {tempo_bpm} BPM"))
    }

    fn set_ref_freq(&mut self, args: &str) -> Result<String, String> {
        let tokens: Vec<_> = args.split_whitespace().collect();
        if tokens.len() != 1 {
            return Err(ref_freq_usage().to_owned());
        }

        let hz = tokens[0]
            .parse::<f32>()
            .map_err(|_| "reference frequency must be a finite positive Hz value".to_owned())?;
        if !hz.is_finite() || hz <= 0.0 {
            return Err("reference frequency must be a finite positive Hz value".to_owned());
        }

        self.engine
            .enqueue(EngineCommand::SetReferenceFrequency(hz))
            .map_err(|error| error.to_string())?;
        self.reference_frequency_hz = hz;
        Ok(format!("reference frequency set to {hz} Hz"))
    }

    fn load_sample_directory(&mut self, args: &str) -> Result<String, String> {
        if args.is_empty() {
            return Err(samples_usage().to_owned());
        }
        let directory = PathBuf::from(args);
        let sample_bank =
            Arc::new(load_sample_bank_from_directory(&directory).map_err(|error| error.to_string())?);
        let available_tokens = sample_bank.available_tokens();
        self.sample_bank = Arc::clone(&sample_bank);
        self.sample_directory = Some(directory.clone());
        self.start_sample_watcher(&directory)?;
        self.engine
            .enqueue(EngineCommand::ReplaceSampleBank(sample_bank))
            .map_err(|error| error.to_string())?;
        Ok(format!(
            "loaded sample overrides from `{}` ({})",
            directory.display(),
            available_tokens.join(", ")
        ))
    }

    fn import_command(&mut self, args: &str) -> Result<String, String> {
        let tokens: Vec<_> = args.split_whitespace().collect();
        match tokens.as_slice() {
            ["stems", directory @ ..] if !directory.is_empty() => {
                self.import_stems(&directory.join(" "))
            }
            _ => Err(import_usage().to_owned()),
        }
    }

    fn import_stems(&mut self, raw_directory: &str) -> Result<String, String> {
        let directory = PathBuf::from(trim_quoted_arg(raw_directory));
        if directory.as_os_str().is_empty() {
            return Err(import_usage().to_owned());
        }

        let stem_paths = collect_stem_wav_paths(&directory)?;
        if stem_paths.is_empty() {
            return Err(format!(
                "no stem WAV files found in `{}`",
                directory.display()
            ));
        }

        let mut reserved_names = BTreeSet::new();
        let mut stems = Vec::with_capacity(stem_paths.len());
        for path in &stem_paths {
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| format!("stem path `{}` is not valid Unicode", path.display()))?;
            let binding_name = self.unique_import_binding_name(stem, &reserved_names);
            reserved_names.insert(binding_name.clone());
            stems.push(ImportedStem {
                binding_name,
                token: sample_token_name(stem),
            });
        }

        let sample_bank =
            Arc::new(load_sample_bank_from_directory(&directory).map_err(|error| error.to_string())?);

        let mut bindings = self.bindings.clone();
        let mut type_bindings = self.type_bindings.clone();
        let mut mixer = self.mixer.clone();
        for stem in &stems {
            let pattern = crate::value::SamplePatternValue::atom(&stem.token);
            bindings.insert(stem.binding_name.clone(), Value::SamplePattern(pattern));
            type_bindings.insert(stem.binding_name.clone(), Type::pattern(Type::Sample));
        }
        for stem in &stems {
            mixer.new_track(&stem.binding_name)?;
            mixer.bind_track(&stem.binding_name, &stem.binding_name, &bindings)?;
        }

        self.bindings = bindings;
        self.type_bindings = type_bindings;
        self.mixer = mixer;
        self.sample_bank = Arc::clone(&sample_bank);
        self.sample_directory = Some(directory.clone());
        self.start_sample_watcher(&directory)?;
        self.engine
            .enqueue(EngineCommand::ReplaceSampleBank(sample_bank))
            .map_err(|error| error.to_string())?;
        self.enqueue_mixer_snapshot()?;
        if let Some(stem) = stems.last() {
            self.pattern_display.borrow_mut().last_loaded_pattern_name =
                Some(stem.binding_name.clone());
        }

        let names = stems
            .iter()
            .map(|stem| stem.binding_name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        Ok(format!(
            "imported {} stem(s) from `{}` ({names})",
            stems.len(),
            directory.display()
        ))
    }

    fn unique_import_binding_name(&self, stem: &str, reserved_names: &BTreeSet<String>) -> String {
        let base = stem_binding_name(stem);
        let mut candidate = base.clone();
        let mut suffix = 2_u32;
        while self.bindings.contains_key(&candidate)
            || self.type_bindings.contains_key(&candidate)
            || self.mixer.contains_routing_name(&candidate)
            || reserved_names.contains(&candidate)
            || is_reserved_routing_name(&candidate)
        {
            candidate = format!("{base}_{suffix}");
            suffix = suffix.saturating_add(1);
        }
        candidate
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
        if path.as_os_str().is_empty() {
            return Err(open_usage().to_owned());
        }
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

        if self.has_voice_bindings() {
            self.sync_graph_voice_programs()?;
        }

        if let Some(name) = last_binding_name
            && let Some(value) = self.bindings.get(&name).cloned()
        {
            self.push_pattern_update(&name, &value)?;
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
            Arc::new(load_sample_bank_from_directory(&directory).map_err(|error| error.to_string())?);
        let available_tokens = sample_bank.available_tokens();
        self.sample_bank = Arc::clone(&sample_bank);
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

    fn start_sample_watcher(&mut self, directory: &Path) -> Result<(), String> {
        let watcher = SampleLibraryWatcher::spawn(directory, SampleLibraryWatcherConfig::default())
            .map_err(|error| error.to_string())?;
        self.sample_watcher = Some(watcher);
        Ok(())
    }

    fn poll_sample_watcher(&mut self) -> Result<(), String> {
        let Some(watcher) = self.sample_watcher.as_mut() else {
            return Ok(());
        };
        let mut latest = None;
        while let Some(reload) = watcher.try_recv() {
            latest = Some(reload);
        }
        let Some(reload) = latest else {
            return Ok(());
        };

        for issue in reload.errors() {
            eprintln!(
                "sample hot reload issue at `{}`: {}",
                issue.path(),
                issue.message()
            );
        }

        let sample_bank = Arc::new(reload.bank().clone());
        self.sample_bank = Arc::clone(&sample_bank);
        self.engine
            .enqueue(EngineCommand::ReplaceSampleBank(sample_bank))
            .map_err(|error| error.to_string())
    }

    fn eval_track_command(&mut self, args: &str) -> Result<String, String> {
        let tokens: Vec<_> = args.split_whitespace().collect();
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
        let tokens: Vec<_> = args.split_whitespace().collect();
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
        let tokens: Vec<_> = args.split_whitespace().collect();
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
        let tokens: Vec<_> = args.split_whitespace().collect();
        match tokens.as_slice() {
            ["in", "list"] => Self::list_midi_inputs(),
            ["in", "connect", port @ ..] if !port.is_empty() => {
                self.connect_midi_input(&port.join(" "))
            }
            ["in", "disconnect"] => self.disconnect_midi_input(),
            ["in", "map-note", note, binding_name] => self.map_midi_note(note, binding_name),
            ["in", "unmap-note", note] => self.unmap_midi_note(note),
            ["list"] => Self::list_midi_outputs(),
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

    fn list_midi_inputs() -> Result<String, String> {
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

    fn list_midi_outputs() -> Result<String, String> {
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

    fn send_midi_binding(&self, binding_name: &str, channel: u8) -> Result<String, String> {
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
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let note = event.value.round().clamp(0.0, 127.0) as u8;
            let start = f64::from(event.part.start());
            let end = f64::from(event.part.end());
            if end > start {
                midi_events.push((start, true, note));
                midi_events.push((end, false, note));
            }
        }
        midi_events.sort_unstable_by(|left, right| {
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
                let target_time =
                    Duration::try_from_secs_f64(f64::max(0.0, offset_in_cycle * seconds_per_cycle))
                        .unwrap_or(Duration::MAX);
                let elapsed = start.elapsed();
                if target_time > elapsed {
                    if let Some(sleep_time) = target_time.checked_sub(elapsed) {
                        thread::sleep(sleep_time);
                    } else {
                        thread::sleep(Duration::from_millis(1));
                    }
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

    /// Rebuilds the engine's graph voice bank from every `voice { ... }`
    /// binding and enqueues it for adoption at the next cycle boundary.
    ///
    /// The bank (built-ins plus one program per voice binding, keyed by the
    /// binding name) is compiled and pre-warmed here on the language thread —
    /// never on the audio thread — mirroring the sample bank swap discipline
    /// (ADR 0009/0010).
    fn sync_graph_voice_programs(&mut self) -> Result<(), String> {
        let specs = self.graph_voice_specs()?;
        let bank = GraphVoiceBank::with_user_programs(self.engine.sample_rate_hz(), specs);
        self.engine
            .enqueue(EngineCommand::ReplaceGraphVoicePrograms(bank))
            .map_err(|error| format!("failed to enqueue graph voice programs: {error}"))
    }

    /// Compiles every `voice { ... }` binding to its graph voice program spec,
    /// keyed by the binding name — the same set the live engine's bank is
    /// built from, reused by offline stem export.
    fn graph_voice_specs(&self) -> Result<Vec<GraphVoiceSpec>, String> {
        let mut specs = Vec::new();
        for (name, value) in &self.bindings {
            if let Value::Voice(voice) = value {
                specs.push(voice.to_spec(name).map_err(|error| error.to_string())?);
            }
        }
        Ok(specs)
    }

    fn has_voice_bindings(&self) -> bool {
        self.bindings
            .values()
            .any(|value| matches!(value, Value::Voice(_)))
    }

    fn enqueue_mixer_snapshot(&mut self) -> Result<(), String> {
        // Live playback routes plain sample-pattern tracks through synthetic
        // generators so finite multi-cycle arrangements (e.g. `seq_sections`)
        // advance section by section instead of looping cycle 0 (issue #1446).
        // The returned mapping tells the arrangement driver which pattern to
        // query at each engine cycle boundary.
        let (snapshot, arrangements) = self.mixer.compile_live_snapshot(&self.bindings)?;
        self.engine
            .enqueue(EngineCommand::SwapRoutingSnapshot(snapshot))
            .map_err(|error| format!("failed to enqueue routing snapshot swap: {error}"))?;

        if self.mixer.has_explicit_bound_tracks() {
            let mut display = self.pattern_display.borrow_mut();
            display.active_pattern_name = None;
            display.pending_pattern_name = None;
            display.pending_enqueued_after_publish = None;
        }

        self.register_arrangement_generators(arrangements)
    }

    /// Adopts the live arrangement-generator mapping from `compile_live_snapshot`
    /// and primes the currently playing cycle so playback is correct
    /// immediately — including headless REPL playback that never ticks the TUI
    /// (issue #1446). Subsequent cycles are delivered by [`Self::poll_arrangements`].
    fn register_arrangement_generators(
        &mut self,
        arrangements: Vec<(GeneratorId, String)>,
    ) -> Result<(), String> {
        self.arrangement_generators = arrangements.into_iter().collect();
        self.arrangement_last_cycle_start = None;
        self.last_arrangement_cycles.clear();
        if self.arrangement_generators.is_empty() {
            return Ok(());
        }
        let transport = self.engine.transport_snapshot();
        let cycle = current_cycle_index(&transport);
        self.push_all_arrangements(cycle)
    }

    /// Delivers the next cycle of every live multi-cycle arrangement when the
    /// engine has crossed a cycle boundary since the last poll (issue #1446).
    /// Called on every TUI tick alongside [`Self::poll_orca`]; a no-op when no
    /// arrangement is active or the boundary is unchanged. The query and push
    /// run here on the control thread, never the audio thread — the audio path
    /// only adopts the delivered buffer at the boundary.
    ///
    /// # Errors
    ///
    /// Returns a message when querying an arrangement or enqueuing its cycle
    /// fails.
    pub fn poll_arrangements(&mut self) -> Result<(), String> {
        if self.arrangement_generators.is_empty() {
            return Ok(());
        }
        let transport = self.engine.transport_snapshot();
        let cycle_start = transport.current_cycle_start_frame();
        if self.arrangement_last_cycle_start == Some(cycle_start) {
            return Ok(());
        }
        self.arrangement_last_cycle_start = Some(cycle_start);
        // Deliver one cycle ahead so the engine adopts it at the next boundary,
        // exactly as the Orca grid generator is driven (ADR 0009).
        let next_cycle = current_cycle_index(&transport).saturating_add(1);
        self.push_all_arrangements(next_cycle)
    }

    /// Queries `cycle_index` of every registered arrangement and pushes it over
    /// the generator ring.
    fn push_all_arrangements(&mut self, cycle_index: u64) -> Result<(), String> {
        let drivers: Vec<(GeneratorId, String)> = self
            .arrangement_generators
            .iter()
            .map(|(id, name)| (*id, name.clone()))
            .collect();
        for (generator_id, binding_name) in drivers {
            let triggers = self.arrangement_cycle_triggers(&binding_name, cycle_index)?;
            self.push_arrangement_cycle(generator_id, triggers)?;
        }
        Ok(())
    }

    /// Materializes cycle `cycle_index` of the arrangement bound to
    /// `binding_name`, shifted into its own `[0, 1)` window.
    fn arrangement_cycle_triggers(
        &self,
        binding_name: &str,
        cycle_index: u64,
    ) -> Result<Vec<Event<SampleTrigger>>, String> {
        let value = self
            .bindings
            .get(binding_name)
            .ok_or_else(|| format!("no binding named `{binding_name}`"))?;
        let pattern = value
            .as_sample_pattern()
            .ok_or_else(|| format!("binding `{binding_name}` is no longer a sample pattern"))?;
        crate::mixer::materialize_pattern_cycle(pattern, cycle_index, binding_name)
    }

    /// Enqueues one live arrangement cycle over the generator ring. Unlike
    /// [`Self::push_generator_cycle`], it does **not** append to
    /// `generator_cycles`: arrangements are re-materialized fresh for offline
    /// export ([`MixerState::compile_offline_snapshot`]), so recording them here
    /// would double-count and could collide with offline synthetic generator
    /// ids.
    fn push_arrangement_cycle(
        &mut self,
        generator_id: GeneratorId,
        triggers: Vec<Event<SampleTrigger>>,
    ) -> Result<(), String> {
        self.last_arrangement_cycles
            .insert(generator_id, triggers.clone());
        self.engine
            .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::new(
                generator_id,
                triggers,
            )))
            .map_err(|error| format!("failed to enqueue arrangement cycle: {error}"))
    }

    fn push_pattern_update(&mut self, name: &str, value: &Value) -> Result<(), String> {
        if let Value::PluginPattern(_) = value {
            self.mixer.note_sample_binding(name);
            self.pattern_display.borrow_mut().last_loaded_pattern_name = Some(name.to_owned());
            return self.enqueue_mixer_snapshot();
        }

        if let Value::SamplePattern(pattern) = value {
            self.mixer.note_sample_binding(name);
            if self.mixer.has_routing_state() {
                self.pattern_display.borrow_mut().last_loaded_pattern_name = Some(name.to_owned());
                return self.enqueue_mixer_snapshot();
            }

            // A finite multi-cycle arrangement (e.g. `seq_sections`) must advance
            // section by section live, not loop cycle 0 (issue #1446). Route it
            // through the generator seam so the arrangement driver can query and
            // push cycle N at each engine cycle boundary. Plain one-cycle loops
            // keep the lighter `LoadPattern` path below, preserving its
            // active/pending transport bookkeeping.
            if crate::mixer::is_multi_cycle_arrangement(pattern) {
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
            if display.active_pattern_name.is_none()
                && enqueue_publish != 0
                && let Some(last_loaded_pattern_name) = display.last_loaded_pattern_name.clone()
            {
                display.active_pattern_name = Some(last_loaded_pattern_name);
            }
            display.last_loaded_pattern_name = Some(name.to_owned());
            display.pending_pattern_name = Some(name.to_owned());
            display.pending_enqueued_after_publish = Some(enqueue_publish);
        }

        Ok(())
    }

    /// Publishes a materialized `Event<SampleEvent>` list as the sample
    /// pattern binding `name`, replacing any previous binding of that name.
    ///
    /// This is the publication seam for generator surfaces such as the Orca
    /// grid pane (ADR 0008): the caller re-materializes its next cycle and
    /// calls this again at every engine cycle boundary. Publication reuses the
    /// same path as evaluated patterns, so mixer routing and track binding
    /// apply unchanged.
    ///
    /// # Errors
    ///
    /// Returns a message when the engine command queue rejects the update or
    /// the mixer snapshot fails to compile.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_dsp::EngineHandle;
    /// use orpheus_lang::ReplSession;
    /// use orpheus_lang::orca::{OrcaEngine, materialize_cycle};
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// let mut grid = OrcaEngine::from_rows(&[".D1...", "..:04c"]).unwrap();
    /// let events = materialize_cycle(&mut grid, 4, "tri").unwrap();
    /// session.publish_sample_events("orca", events).unwrap();
    /// assert!(
    ///     session
    ///         .binding_summaries()
    ///         .iter()
    ///         .any(|summary| summary == "orca: Pattern<Sample>")
    /// );
    /// ```
    pub fn publish_sample_events(
        &mut self,
        name: &str,
        events: Vec<orpheus_pattern::Event<SampleEvent>>,
    ) -> Result<(), String> {
        let value = Value::SamplePattern(SamplePatternValue::from_events(events));
        self.bindings.insert(name.to_owned(), value.clone());
        self.type_bindings
            .insert(name.to_owned(), Type::pattern(Type::Sample));
        self.push_pattern_update(name, &value)
    }

    /// Binds `name` to the engine generator slot `generator_id` and delivers
    /// the first materialized cycle (ADR 0009).
    ///
    /// This is the v5 publication seam for generator surfaces such as the
    /// Orca grid pane: the engine schedules one delivered cycle buffer per
    /// boundary from a dedicated [`orpheus_dsp::TrackSource::Generator`]
    /// track, so consecutive cycles play back-to-back and events whose
    /// `whole` extends past the cycle end sustain across the boundary. A
    /// display binding is registered under `name` so the grid shows up in
    /// binding summaries and mixer routing like any pattern; the audible
    /// events flow over [`EngineCommand::PushGeneratorCycle`], not from that
    /// binding.
    ///
    /// # Errors
    ///
    /// Returns a message when the mixer snapshot fails to compile or the
    /// engine command queue rejects the routing swap or cycle buffer.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_dsp::EngineHandle;
    /// use orpheus_lang::ReplSession;
    /// use orpheus_lang::orca::{ORCA_GENERATOR_ID, OrcaEngine, materialize_cycle};
    ///
    /// let mut session = ReplSession::with_engine(EngineHandle::stub());
    /// let mut grid = OrcaEngine::from_rows(&[".D1...", "..:04c"]).unwrap();
    /// let events = materialize_cycle(&mut grid, 4, "tri").unwrap();
    /// session
    ///     .start_generator_source("orca", ORCA_GENERATOR_ID, events)
    ///     .unwrap();
    /// assert!(
    ///     session
    ///         .binding_summaries()
    ///         .iter()
    ///         .any(|summary| summary == "orca: Pattern<Sample>")
    /// );
    /// ```
    pub fn start_generator_source(
        &mut self,
        name: &str,
        generator_id: GeneratorId,
        events: Vec<orpheus_pattern::Event<SampleEvent>>,
    ) -> Result<(), String> {
        let value = Value::SamplePattern(SamplePatternValue::from_events(events.clone()));
        self.bindings.insert(name.to_owned(), value);
        self.type_bindings
            .insert(name.to_owned(), Type::pattern(Type::Sample));
        self.mixer.note_generator_binding(name, generator_id);
        self.mixer.note_sample_binding(name);
        self.enqueue_mixer_snapshot()?;
        self.pattern_display.borrow_mut().last_loaded_pattern_name = Some(name.to_owned());
        // Restart the export record from grid start: the first delivered cycle
        // (below) becomes exported cycle 0 (ADR 0009 addendum).
        self.generator_cycles.insert(generator_id, Vec::new());
        self.push_generator_cycle(generator_id, events)
    }

    /// Delivers the next materialized cycle for a running generator source.
    ///
    /// The engine adopts the buffer at its next cycle boundary; delivering
    /// one cycle ahead therefore plays consecutive grid cycles back-to-back.
    /// When no fresh buffer arrives in time the engine loops the previous
    /// one, so a stalled UI degrades to repetition rather than silence.
    ///
    /// # Errors
    ///
    /// Returns a message when the engine command queue is full.
    pub fn push_generator_cycle(
        &mut self,
        generator_id: GeneratorId,
        events: Vec<orpheus_pattern::Event<SampleEvent>>,
    ) -> Result<(), String> {
        let triggers: Vec<Event<SampleTrigger>> = events
            .into_iter()
            .map(|event| orpheus_pattern::Event {
                whole: event.whole,
                part: event.part,
                value: sample_trigger_from_event(&event.value),
            })
            .collect();
        self.record_generator_cycle(generator_id, triggers.clone().into_boxed_slice());
        self.engine
            .enqueue(EngineCommand::PushGeneratorCycle(GeneratorCycle::new(
                generator_id,
                triggers,
            )))
            .map_err(|error| format!("failed to enqueue generator cycle: {error}"))
    }

    /// Appends a delivered generator cycle buffer to the per-slot export record
    /// (ADR 0009 addendum), bounded by [`MAX_RECORDED_GENERATOR_CYCLES`].
    fn record_generator_cycle(
        &mut self,
        generator_id: GeneratorId,
        triggers: Box<[Event<SampleTrigger>]>,
    ) {
        let recorded = self.generator_cycles.entry(generator_id).or_default();
        if recorded.len() < MAX_RECORDED_GENERATOR_CYCLES {
            recorded.push(triggers);
        }
    }

    /// Builds one [`GeneratorCycleSpec`] per active generator slot for offline
    /// stem export, mirroring [`Self::graph_voice_specs`]. Each spec carries up
    /// to `cycle_count` recorded cycle buffers in delivery order (from grid
    /// start); the offline renderer loops the last buffer if fewer were
    /// recorded, matching the live engine's starvation semantics.
    fn generator_cycle_specs(&self, cycle_count: u64) -> Vec<GeneratorCycleSpec> {
        let max = usize::try_from(cycle_count).unwrap_or(usize::MAX);
        self.generator_cycles
            .iter()
            .map(|(generator_id, recorded)| GeneratorCycleSpec {
                generator_id: *generator_id,
                cycles: recorded.iter().take(max).cloned().collect(),
            })
            .collect()
    }

    /// Silences a generator source at the next cycle boundary.
    ///
    /// Delivers an empty cycle buffer (which then loops, keeping the
    /// generator silent) and clears the display binding's events. The
    /// generator registration and routing stay in place so a restart is a
    /// plain [`Self::push_generator_cycle`] away.
    ///
    /// # Errors
    ///
    /// Returns a message when the engine command queue is full.
    pub fn stop_generator_source(
        &mut self,
        name: &str,
        generator_id: GeneratorId,
    ) -> Result<(), String> {
        self.bindings.insert(
            name.to_owned(),
            Value::SamplePattern(SamplePatternValue::from_events(Vec::new())),
        );
        self.push_generator_cycle(generator_id, Vec::new())
    }

    /// Compiles a summary of all active bindings and their inferred types.
    ///
    /// This is used heavily by the `:env` REPL command and the live TUI dashboard to
    /// provide a clear inventory of all currently evaluated user variables, allowing them
    /// to inspect the global state without needing to remember what they typed.
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

    /// Exposes the name of the most recently evaluated and loaded pattern for assertions.
    /// Useful when verifying that REPL or script execution resulted in the expected active bindings.
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
        } else if display.active_pattern_name.is_none()
            && snapshot.current_frame() != 0
            && let Some(last_loaded_pattern_name) = display.last_loaded_pattern_name.clone()
        {
            display.active_pattern_name = Some(last_loaded_pattern_name);
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
    /// session.render_test_block_for_tui(1);
    ///
    /// let view = session.mixer_view();
    /// assert!(view.has_pending_routing());
    /// ```
    pub fn mixer_view(&self) -> MixerView {
        let snapshot = self.engine.transport_snapshot();
        let meters = self.meter_view();
        MixerView {
            has_pending_routing: snapshot.has_pending_routing(),
            summary: self.mixer.render_summary(),
            tui_summary: self.mixer.render_tui_summary(meters.peaks()),
        }
    }

    /// Returns a UI-readable per-track level-meter snapshot (ADR 0013).
    ///
    /// Mirrors [`ReplSession::transport_view`]: the meters are updated on the
    /// audio thread and read here lock-free, so the TUI can render live level
    /// bars without interrupting audio generation.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_lang::ReplSession;
    /// use orpheus_dsp::EngineHandle;
    ///
    /// let session = ReplSession::with_engine(EngineHandle::stub());
    /// let meters = session.meter_view();
    /// assert_eq!(meters.track_peak(0), 0.0);
    /// ```
    #[must_use]
    pub fn meter_view(&self) -> LevelSnapshot {
        self.engine.meter_snapshot()
    }

    #[doc(hidden)]
    pub fn render_test_block_for_tui(&mut self, frames: u64) -> Vec<f32> {
        let _ = self.apply_midi_note_mappings();
        let _ = self.poll_sample_watcher();
        self.engine.render_test_block(frames)
    }

    #[doc(hidden)]
    pub fn frames_until_boundary_for_tui(&self) -> u64 {
        self.engine.frames_until_boundary_for_test()
    }
}

/// The absolute cycle index the engine is currently in, from its transport
/// snapshot. Returns 0 before the frames-per-cycle clock is initialized.
const fn current_cycle_index(transport: &orpheus_dsp::TransportSnapshot) -> u64 {
    let frames_per_cycle = transport.frames_per_cycle();
    if frames_per_cycle == 0 {
        0
    } else {
        transport.current_cycle_start_frame() / frames_per_cycle
    }
}

fn success_banner(name: &str, value: &Value, ty: &Type) -> String {
    format!("bound {name} = {value}: {ty}")
}

const fn render_usage() -> &'static str {
    "usage: :render <binding> <path> [cycles]"
}

const fn export_usage() -> &'static str {
    "usage: :export <binding> <path> [cycles] | :export stems [cycles] [--buses] | :export master <path> [cycles] [--no-buses]"
}

const fn master_export_usage() -> &'static str {
    "usage: :export master <path> [cycles] [--no-buses]"
}

const fn roll_usage() -> &'static str {
    "usage: :roll <binding> [cycles] [steps_per_cycle]"
}

const fn stats_usage() -> &'static str {
    "usage: :stats <binding> [cycles]"
}

const fn explain_usage() -> &'static str {
    "usage: :explain <binding>"
}

const fn tempo_usage() -> &'static str {
    "usage: :tempo <bpm>"
}

const fn ref_freq_usage() -> &'static str {
    "usage: :ref_freq <hz>"
}

const fn samples_usage() -> &'static str {
    "usage: :samples <directory>"
}

const fn import_usage() -> &'static str {
    "usage: :import stems <directory>"
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

const fn undo_usage() -> &'static str {
    "usage: :undo"
}

const fn redo_usage() -> &'static str {
    "usage: :redo"
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImportedStem {
    binding_name: String,
    token: String,
}

fn collect_stem_wav_paths(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    let entries = std::fs::read_dir(directory).map_err(|source| {
        format!(
            "failed to read stem directory `{}`: {}",
            directory.display(),
            readable_io_error(&source)
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| {
            format!(
                "failed to read stem directory `{}`: {}",
                directory.display(),
                readable_io_error(&source)
            )
        })?;
        let path = entry.path();
        if path.is_file() && is_stem_wav_path(&path) {
            paths.push(path);
        }
    }

    paths.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    Ok(paths)
}

fn is_stem_wav_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "wav" | "wave"))
}

fn stem_binding_name(stem: &str) -> String {
    let mut name = String::with_capacity(stem.len());
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            name.push(ch.to_ascii_lowercase());
        } else {
            name.push('_');
        }
    }

    let mut name = name.trim_matches('_').to_owned();
    if name.is_empty() {
        name.push_str("stem");
    }
    if !name
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
    {
        name.insert_str(0, "stem_");
    }
    if is_reserved_routing_name(&name) {
        name.push_str("_stem");
    }
    name
}

fn sample_token_name(stem: &str) -> String {
    let mut token = String::with_capacity(stem.len());
    let mut previous_was_separator = false;
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            token.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            token.push('_');
            previous_was_separator = true;
        }
    }

    let token = token.trim_matches('_');
    if token.is_empty() {
        "sample".to_owned()
    } else {
        token.to_owned()
    }
}

fn is_reserved_routing_name(name: &str) -> bool {
    matches!(name, "main" | "master")
}

fn readable_io_error(source: &std::io::Error) -> String {
    match source.kind() {
        std::io::ErrorKind::NotFound => "file not found".to_owned(),
        std::io::ErrorKind::PermissionDenied => "permission denied".to_owned(),
        _ => source.to_string(),
    }
}

#[cfg(test)]
mod tests {

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

    fn sample_tokens(session: &ReplSession, binding_name: &str) -> Vec<String> {
        session
            .bindings
            .get(binding_name)
            .and_then(crate::Value::as_sample_pattern)
            .unwrap_or_else(|| panic!("expected `{binding_name}` to be a sample pattern"))
            .query_unit()
            .unwrap_or_else(|error| panic!("querying `{binding_name}` should succeed: {error}"))
            .into_iter()
            .map(|event| event.value.sample().to_owned())
            .collect()
    }

    fn constant_number(session: &ReplSession, binding_name: &str) -> f64 {
        session
            .bindings
            .get(binding_name)
            .and_then(crate::Value::as_number_pattern)
            .unwrap_or_else(|| panic!("expected `{binding_name}` to be a number pattern"))
            .constant_value()
            .unwrap_or_else(|_| panic!("expected `{binding_name}` to be constant"))
    }

    fn assert_constant_number(session: &ReplSession, binding_name: &str, expected: f64) {
        let actual = constant_number(session, binding_name);
        assert!(
            (actual - expected).abs() < f64::EPSILON,
            "expected `{binding_name}` to be {expected}, got {actual}"
        );
    }

    #[test]
    fn help_command_prints_table_of_commands() {
        let mut session = ReplSession::new();
        let output = session.eval_line(":help").unwrap();
        assert!(output.contains("REPL Commands:"));
        assert!(output.contains(":env"));
        assert!(output.contains(":explain <binding>"));
        assert!(output.contains("─")); // comfy-table border char
    }

    #[test]
    fn help_command_rejects_arguments() {
        let mut session = ReplSession::new();
        let error = session.eval_line(":help me").unwrap_err();
        assert_eq!(error, "usage: :help (no arguments)");
    }

    #[test]
    fn undo_and_redo_restore_recent_binding_states() {
        let mut session = ReplSession::new();

        assert_eq!(
            session.eval_line(":undo"),
            Err("nothing to undo".to_owned())
        );
        session.eval_line("drums = bd").unwrap();
        session.eval_line("drums = sn").unwrap();
        assert_eq!(sample_tokens(&session, "drums"), vec!["sn"]);

        assert_eq!(
            session.eval_line(":undo").unwrap(),
            "undid last session change"
        );
        assert_eq!(sample_tokens(&session, "drums"), vec!["bd"]);

        assert_eq!(session.eval_line(":redo").unwrap(), "redid session change");
        assert_eq!(sample_tokens(&session, "drums"), vec!["sn"]);
    }

    #[test]
    fn undo_restores_mixer_routing_and_redo_reapplies_it() {
        let mut session = ReplSession::new();

        session.eval_line("drums = bd").unwrap();
        session.eval_line(":track new kit").unwrap();
        session.eval_line(":track bind kit drums").unwrap();
        session.eval_line(":track level kit 0.25").unwrap();
        assert!(session.eval_line(":mixer").unwrap().contains("0.25"));

        assert_eq!(
            session.eval_line(":undo").unwrap(),
            "undid last session change"
        );
        let undone = session.eval_line(":mixer").unwrap();
        assert!(undone.contains("kit"));
        assert!(undone.contains("1.00"));
        assert!(!undone.contains("0.25"));

        assert_eq!(session.eval_line(":redo").unwrap(), "redid session change");
        assert!(session.eval_line(":mixer").unwrap().contains("0.25"));
    }

    #[test]
    fn redo_stack_is_cleared_by_new_session_change_after_undo() {
        let mut session = ReplSession::new();

        session.eval_line("x = 1").unwrap();
        session.eval_line("x = 2").unwrap();
        session.eval_line(":undo").unwrap();
        assert_constant_number(&session, "x", 1.0);

        session.eval_line("x = 3").unwrap();
        assert_eq!(
            session.eval_line(":redo"),
            Err("nothing to redo".to_owned())
        );
        assert_constant_number(&session, "x", 3.0);
    }

    #[test]
    fn history_keeps_only_the_last_fifty_session_changes() {
        let mut session = ReplSession::new();

        for value in 0..60 {
            session.eval_line(&format!("x = {value}")).unwrap();
        }

        for _ in 0..50 {
            session.eval_line(":undo").unwrap();
        }

        assert_constant_number(&session, "x", 9.0);
        assert_eq!(
            session.eval_line(":undo"),
            Err("nothing to undo".to_owned())
        );
    }

    #[test]
    fn undo_restoration_waits_for_the_next_cycle_boundary() {
        let mut session = ReplSession::new();

        session.eval_line(":tempo 48000").unwrap();
        session.eval_line("drums = bd").unwrap();
        let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
        session.eval_line("drums = sn").unwrap();
        let _ = session.render_test_block_for_tui(1);

        session.eval_line(":undo").unwrap();
        let _ = session.render_test_block_for_tui(1);

        assert!(!session.engine.swap_applied_before_boundary());
        let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
        assert!(!session.engine.swap_applied_before_boundary());
    }

    /// The live twin of the offline master-render fix (#1458): a finite
    /// multi-cycle arrangement must advance section by section during live
    /// playback, not loop cycle 0 forever (issue #1446). Publishing binds the
    /// arrangement to a synthetic generator and primes cycle 0; driving the
    /// cycle-advance seam (the same one that drives Orca generators) must
    /// deliver each later section's distinct events.
    #[test]
    fn live_arrangement_advances_section_by_section() {
        let mut session = ReplSession::new();

        // Three one-cycle sections with distinguishable tokens: bd, sn, cp.
        session
            .eval_line("song = seq_sections(section(bd, 1), section(sn, 1), section(cp, 1))")
            .unwrap();

        // Publishing routes the arrangement through the generator seam and
        // primes cycle 0 (the intro) immediately.
        let arrangement_id = *session
            .arrangement_generators
            .keys()
            .next()
            .expect("the arrangement should register as a live generator");
        let cycle0 = session
            .last_arrangement_cycles
            .get(&arrangement_id)
            .cloned()
            .expect("cycle 0 delivered at publish");

        session.eval_line(":play").unwrap();

        // While in cycle 0, the driver delivers cycle 1 one boundary ahead.
        session.poll_arrangements().unwrap();
        let cycle1 = session
            .last_arrangement_cycles
            .get(&arrangement_id)
            .cloned()
            .expect("cycle 1 delivered");

        // Cross one engine cycle boundary, then poll again for cycle 2.
        let before = session
            .engine
            .transport_snapshot()
            .current_cycle_start_frame();
        loop {
            let _ =
                session.render_test_block_for_tui(session.frames_until_boundary_for_tui().max(1));
            if session
                .engine
                .transport_snapshot()
                .current_cycle_start_frame()
                != before
            {
                break;
            }
        }
        session.poll_arrangements().unwrap();
        let cycle2 = session
            .last_arrangement_cycles
            .get(&arrangement_id)
            .cloned()
            .expect("cycle 2 delivered");

        // The three delivered cycles must differ — the arrangement advances
        // (bd -> sn -> cp) rather than looping cycle 0.
        assert_ne!(cycle0, cycle1, "cycle 0 (bd) and cycle 1 (sn) must differ");
        assert_ne!(cycle1, cycle2, "cycle 1 (sn) and cycle 2 (cp) must differ");
        assert_ne!(cycle0, cycle2, "cycle 0 (bd) and cycle 2 (cp) must differ");
        assert!(
            !cycle0.is_empty() && !cycle1.is_empty() && !cycle2.is_empty(),
            "each delivered section should carry an event"
        );
    }

    /// A plain one-cycle loop must keep looping that single cycle live: the
    /// driver queries cycle N of a one-cycle pattern and gets the same events
    /// every cycle. This pins that the arrangement fix does not corrupt the
    /// common loop case.
    #[test]
    fn live_single_cycle_loop_keeps_looping() {
        let mut session = ReplSession::new();
        session.eval_line("drums = bd sn").unwrap();

        // A plain loop is not a multi-cycle arrangement, so it stays on the
        // static LoadPattern path and registers no arrangement generator.
        assert!(
            session.arrangement_generators.is_empty(),
            "a one-cycle loop should not be routed through the arrangement seam"
        );
    }

    #[test]
    fn env_command_prints_table_of_bindings() {
        let mut session = ReplSession::new();
        session.eval_line("drums = bd sn cp sn").unwrap();
        session.eval_line("tempo = 120").unwrap();

        let output = session.eval_line(":env").unwrap();
        assert!(output.contains("drums"));
        assert!(output.contains("Pattern<Sample>"));
        assert!(output.contains("tempo"));
        assert!(output.contains("Pattern<Number>"));
        assert!(output.contains("─")); // comfy-table border char
    }

    #[test]
    fn env_command_reports_empty_environment() {
        let mut session = ReplSession::new();
        let output = session.eval_line(":env").unwrap();
        assert_eq!(output, "environment is empty");
    }

    #[test]
    fn eval_line_reuses_prior_bindings() {
        let mut session = ReplSession::new();

        assert_eq!(
            session.eval_line("drums = bd sn cp sn"),
            Ok("bound drums = Pattern<Sample>: Pattern<Sample>".to_owned())
        );
        assert_eq!(
            session.eval_line("copy = drums"),
            Ok("bound copy = Pattern<Sample>: Pattern<Sample>".to_owned())
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

        assert!(mixer.contains("verb @ 0.35"));
        assert!(mixer.contains("dub @ 0.50"));
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

        std::fs::remove_dir_all(directory).unwrap();
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

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn roll_command_prints_ascii_roll() {
        let mut session = ReplSession::new();
        session.eval_line("pattern = bd sn").unwrap();

        let message = session.eval_line(":roll pattern 1 8").unwrap();

        assert!(message.contains("Pattern Roll:"));
        assert!(message.contains("bd"));
        assert!(message.contains("sn"));
        assert!(message.contains("x---...."));
        assert!(message.contains("....x---"));
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
    fn stats_command_returns_tuning_stats() {
        let mut session = ReplSession::new();
        session
            .eval_line("t = tuning(1.0 1.125 1.25 1.5 2.0)")
            .unwrap();

        let message = session.eval_line(":stats t").unwrap();

        assert!(message.contains("Tuning Stats:"));
        assert!(message.contains("Name"));
        assert!(message.contains("tuning"));
        assert!(message.contains("Period"));
        assert!(message.contains("2.000"));
        assert!(message.contains("Ref Semitone"));
        assert!(message.contains("0"));
        assert!(message.contains("Ratios"));
        assert!(message.contains("1.000"));
        assert!(message.contains("1.125"));
        assert!(message.contains("1.250"));
        assert!(message.contains("1.500"));
        assert!(message.contains("2.000"));
        assert!(message.contains("5"));
    }

    #[test]
    fn stats_command_returns_sample_pattern_stats() {
        let mut session = ReplSession::new();
        session.eval_line("pattern = fast(2, bd sn)").unwrap();

        let message = session.eval_line(":stats pattern 2").unwrap();

        assert!(message.contains("pattern"));
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
        assert!(std::fs::metadata(&path).unwrap().len() > 44);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn export_command_exports_number_pattern_to_abc() {
        let mut session = ReplSession::new();
        let path =
            std::env::temp_dir().join(format!("orpheus-export-{}.abc", unique_temp_suffix()));

        session.eval_line("notes = 60 62 64").unwrap();
        let message = session
            .eval_line(&format!(":export notes {} 1", path.display()))
            .unwrap();

        assert!(message.contains("exported `notes`"));
        assert!(path.exists());
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("X:1"));
        assert!(contents.contains("C5 D5 E5 "));

        let _ = std::fs::remove_file(path);
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
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains(
            "start_num,start_den,start_float,end_num,end_den,end_float,sample,gain,pan,rate"
        ));
        assert!(contents.contains("bd"));
        assert!(contents.contains("sn"));

        let _ = std::fs::remove_file(path);
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
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(contents.contains("bd"));
        assert!(contents.contains("sn"));

        let _ = std::fs::remove_file(path);
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
        let contents = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(json["kind"], "sample");
        assert_eq!(json["cycle_count"], 2);
        assert!(json["events"].is_array());
        assert_eq!(json["events"][0]["sample"], "bd");
        assert_eq!(json["events"][1]["sample"], "sn");

        let _ = std::fs::remove_file(path);
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
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(contents.contains("<rect"));

        let _ = std::fs::remove_file(path);
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
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(
            contents.contains("start_num,start_den,start_float,end_num,end_den,end_float,value")
        );
        assert!(contents.contains('1'));
        assert!(contents.contains('2'));

        let _ = std::fs::remove_file(path);
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
        let contents = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&contents).unwrap();
        assert_eq!(json["kind"], "number");
        assert_eq!(json["cycle_count"], 1);
        assert!(json["events"].is_array());
        assert_eq!(json["events"][0]["value"], 1.0);
        assert_eq!(json["events"][1]["value"], 2.0);

        let _ = std::fs::remove_file(path);
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

        let _ = std::fs::remove_dir_all(rendered_dir);
    }

    #[test]
    fn export_stems_command_renders_graph_voice_tracks_audibly() {
        let mut session = ReplSession::new();

        session
            .eval_line(
                "pluck = voice { osc = saw(freq) ; env = adsr(gate, 0.001, 0.02, 0.5, 0.05) ; osc * env }",
            )
            .unwrap();
        session.eval_line("lead = pluck pluck").unwrap();
        session.eval_line(":track new lead_track").unwrap();
        session.eval_line(":track bind lead_track lead").unwrap();

        let message = session.eval_line(":export stems 1").unwrap();
        assert!(message.contains("exported 1 stem(s)"));
        let rendered_dir = parse_exported_stem_dir(&message);
        let path = rendered_dir.join("lead_track.wav");
        assert!(path.exists(), "missing stem `lead_track.wav`");
        let mut reader = hound::WavReader::open(&path).unwrap();
        let nonzero = reader
            .samples::<i16>()
            .map(Result::unwrap)
            .any(|sample| sample != 0);
        assert!(nonzero, "graph voice stem should contain rendered audio");

        let _ = std::fs::remove_dir_all(rendered_dir);
    }

    #[test]
    #[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
    fn master_render_of_reference_song_is_full_length_and_non_silent() {
        use orpheus_dsp::{
            GeneratorCycleSpec, GeneratorId, RoutingSnapshot, SampleTrigger, TrackSource,
            render_routing_snapshot_to_master_wav,
        };
        use orpheus_pattern::{Event, Rational, TimeSpan};

        use crate::export::sample_trigger_from_event;

        // The official reference song is a 36-cycle `seq_sections` arrangement
        // (intro 4, build 4, drop 8, peak 8, melodic 4, breakdown 4, outro 4).
        // At 120 BPM (2s/cycle) that is a 72-second master.
        const CYCLES: u64 = 36;
        const TEMPO_BPM: f32 = 120.0;

        let reference = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("docs")
            .join("examples")
            .join("reference_song.ode");

        let mut session = ReplSession::new();
        session
            .open_file(&reference)
            .expect("reference song should load and evaluate");

        let song = session
            .bindings
            .get("song")
            .and_then(super::Value::as_sample_pattern)
            .expect("`song` must be a sample pattern")
            .clone();

        // Materialize the whole timeline cycle-by-cycle into cycle-local event
        // buffers (the generator seam the offline renderer uses for grid/orca
        // tracks), so the `seq_sections` arrangement advances section by section
        // instead of looping cycle 0 the way a single-cycle snapshot capture
        // would.
        let full = song
            .try_query(
                &TimeSpan::new(
                    Rational::from_integer(0),
                    Rational::from_integer(i64::try_from(CYCLES).unwrap()),
                )
                .unwrap(),
            )
            .expect("querying the full song timeline should succeed");

        let mut per_cycle: Vec<Vec<Event<SampleTrigger>>> =
            (0..CYCLES).map(|_| Vec::new()).collect();
        for event in &full {
            let cycle = event.part.start().numerator() / event.part.start().denominator();
            let Ok(index) = usize::try_from(cycle) else {
                continue;
            };
            if index >= per_cycle.len() {
                continue;
            }
            let shift = |bound: &Rational| {
                Rational::new(
                    i64::try_from(bound.numerator() - cycle * bound.denominator()).unwrap(),
                    i64::try_from(bound.denominator()).unwrap(),
                )
                .unwrap()
            };
            let part = TimeSpan::new(shift(event.part.start()), shift(event.part.end())).unwrap();
            let whole = event
                .whole
                .as_ref()
                .map(|w| TimeSpan::new(shift(w.start()), shift(w.end())).unwrap());
            per_cycle[index].push(Event {
                whole,
                part,
                value: sample_trigger_from_event(&event.value),
            });
        }
        let total_events: usize = per_cycle.iter().map(Vec::len).sum();
        assert!(
            total_events > 0,
            "the reference song timeline produced no events"
        );

        let generator_id = GeneratorId::new(0);
        let generator_cycles = vec![GeneratorCycleSpec {
            generator_id,
            cycles: per_cycle.into_iter().map(Vec::into_boxed_slice).collect(),
        }];
        let graph_voice_specs = session.graph_voice_specs().unwrap();

        let out_path = std::env::var_os("ORPHEUS_REFERENCE_MASTER_OUT").map_or_else(
            || {
                std::env::temp_dir().join(format!(
                    "orpheus-reference-master-{}.wav",
                    unique_temp_suffix()
                ))
            },
            PathBuf::from,
        );
        let keep_output = std::env::var_os("ORPHEUS_REFERENCE_MASTER_OUT").is_some();

        let make_snapshot = |level: f32| {
            RoutingSnapshot::builder()
                .track_with_source_and_mix(
                    "song",
                    TrackSource::Generator(generator_id),
                    level,
                    0.0,
                    false,
                )
                .route("song", "master")
                .build()
                .unwrap()
        };

        // Render once at unity to measure the raw summed peak, then re-render
        // with just enough headroom that the master never clips full scale.
        let (_, probe) = render_routing_snapshot_to_master_wav(
            &make_snapshot(1.0),
            CYCLES,
            TEMPO_BPM,
            &session.sample_bank,
            &graph_voice_specs,
            &generator_cycles,
            &out_path,
            true,
        )
        .unwrap();
        let headroom = if probe.peak > 0.891 {
            0.891 / probe.peak
        } else {
            1.0
        };

        let (written, stats) = render_routing_snapshot_to_master_wav(
            &make_snapshot(headroom),
            CYCLES,
            TEMPO_BPM,
            &session.sample_bank,
            &graph_voice_specs,
            &generator_cycles,
            &out_path,
            true,
        )
        .unwrap();

        let mut reader = hound::WavReader::open(&written).unwrap();
        let spec = reader.spec();
        assert_eq!(spec.channels, 2, "master must be stereo");
        assert_eq!(spec.sample_rate, 48_000);
        let samples: Vec<i16> = reader.samples::<i16>().map(Result::unwrap).collect();
        assert!(!samples.is_empty());
        let duration = (samples.len() / 2) as f64 / f64::from(spec.sample_rate);

        let full_scale = f64::from(i16::MAX);
        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
        let peak_norm = f64::from(peak) / full_scale;
        let square_sum: f64 = samples
            .iter()
            .map(|s| {
                let v = f64::from(*s);
                v * v
            })
            .sum();
        let rms = (square_sum / samples.len() as f64).sqrt() / full_scale;
        let clipped = samples
            .iter()
            .filter(|s| s.unsigned_abs() >= 32_767)
            .count();
        let clipped_pct = clipped as f64 / samples.len() as f64 * 100.0;

        println!(
            "reference master: path={} bytes={} duration={duration:.2}s sr={} ch={} \
             peak={peak_norm:.4} rms={rms:.4} clipped={clipped_pct:.4}% \
             reported_peak={:.4} headroom={headroom:.3} events={total_events}",
            written.display(),
            std::fs::metadata(&written).unwrap().len(),
            spec.sample_rate,
            spec.channels,
            stats.peak,
        );

        // No NaN/Inf is representable in i16 PCM; the reported float peak must be
        // finite and within full scale.
        assert!(stats.peak.is_finite(), "reported peak must be finite");
        assert!(
            stats.peak <= 1.0,
            "master must not clip (peak {})",
            stats.peak
        );
        assert!(clipped_pct < 0.1, "master should be essentially clip-free");
        assert!(
            (60.0..=120.0).contains(&duration),
            "duration {duration}s should sit in the 60-120s window"
        );
        assert!(rms > 0.02, "master should carry strong energy (rms {rms})");

        // Non-silent across the WHOLE file: every window carries real energy
        // (well above the numerical noise floor), so the piece plays end to end
        // rather than trailing off into silence after a few cycles. The loud
        // sections (drop/peak) and the sparse ones (intro/outro) differ widely,
        // which additionally proves the `seq_sections` timeline actually
        // advances instead of looping a single cycle.
        let windows = 18;
        let window = samples.len() / windows;
        let mut window_rms = Vec::with_capacity(windows);
        for w in 0..windows {
            let slice = &samples[w * window..(w + 1) * window];
            let wsum: f64 = slice
                .iter()
                .map(|s| {
                    let v = f64::from(*s);
                    v * v
                })
                .sum();
            let wrms = (wsum / slice.len() as f64).sqrt() / full_scale;
            assert!(
                wrms > 0.0008,
                "window {w}/{windows} is effectively silent (rms {wrms})"
            );
            window_rms.push(wrms);
        }
        let loudest = window_rms.iter().copied().fold(0.0_f64, f64::max);
        let quietest = window_rms.iter().copied().fold(f64::MAX, f64::min);
        assert!(
            loudest > 0.05,
            "at least one section must be clearly loud (loudest window rms {loudest})"
        );
        assert!(
            loudest > quietest * 4.0,
            "the arrangement should have real section dynamics, not a single looped cycle \
             (loudest {loudest}, quietest {quietest})"
        );

        if !keep_output {
            let _ = std::fs::remove_file(&written);
        }
    }

    #[test]
    fn export_stems_command_renders_generator_track_audibly() {
        use crate::orca::{ORCA_GENERATOR_ID, ORCA_PATTERN_NAME, OrcaEngine, materialize_cycle};

        let mut session = ReplSession::new();

        // A banging note grid: `D1` fires the `:04c` note every frame.
        let mut grid = OrcaEngine::from_rows(&[".D1...", "..:04c"]).unwrap();
        let events = materialize_cycle(&mut grid, 16, "tri").unwrap();
        assert!(!events.is_empty(), "the grid must emit notes to export");

        session
            .start_generator_source(ORCA_PATTERN_NAME, ORCA_GENERATOR_ID, events)
            .unwrap();
        session.eval_line(":track new orca_track").unwrap();
        session.eval_line(":track bind orca_track orca").unwrap();

        let message = session.eval_line(":export stems 1").unwrap();
        assert!(message.contains("exported"), "got: {message}");
        let rendered_dir = parse_exported_stem_dir(&message);
        let path = rendered_dir.join("orca_track.wav");
        assert!(path.exists(), "missing generator stem `orca_track.wav`");
        let mut reader = hound::WavReader::open(&path).unwrap();
        let nonzero = reader
            .samples::<i16>()
            .map(Result::unwrap)
            .any(|sample| sample != 0);
        assert!(
            nonzero,
            "generator stem should contain audio from its delivered cycles"
        );

        let _ = std::fs::remove_dir_all(rendered_dir);
    }

    #[test]
    fn export_stems_command_can_include_bus_stems() {
        let mut session = ReplSession::new();

        session.eval_line("drums = bd").unwrap();
        session.eval_line(":track new drums_track").unwrap();
        session.eval_line(":track bind drums_track drums").unwrap();
        session.eval_line(":bus new verb").unwrap();
        session
            .eval_line(":bus fx verb reverb size=0.75 damp=0.35 wet=1.0")
            .unwrap();
        session.eval_line(":send drums_track verb 1.0").unwrap();

        let message = session.eval_line(":export stems 1 --buses").unwrap();
        assert!(message.contains("exported 2 stem(s)"));
        let rendered_dir = parse_exported_stem_dir(&message);
        assert!(rendered_dir.join("drums_track.wav").exists());
        assert!(rendered_dir.join("verb_bus.wav").exists());

        let _ = std::fs::remove_dir_all(rendered_dir);
    }

    #[test]
    fn import_stems_command_loads_wavs_as_bound_tracks() {
        let mut session = ReplSession::new();
        let directory = temp_directory("import-stems");
        write_wav(directory.join("Drum Stem.wav"), &[0.5, 0.0, 0.0, 0.0]);
        write_wav(directory.join("bass-track.wav"), &[0.25, 0.0, 0.0, 0.0]);
        std::fs::write(directory.join("notes.txt"), "not a stem").unwrap();

        let message = session
            .eval_line(&format!(":import stems {}", directory.display()))
            .unwrap();

        assert!(message.contains("imported 2 stem(s)"));
        assert!(message.contains("bass_track"));
        assert!(message.contains("drum_stem"));
        assert_eq!(
            session.binding_summaries(),
            vec![
                "bass_track: Pattern<Sample>".to_owned(),
                "drum_stem: Pattern<Sample>".to_owned()
            ]
        );

        let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
        assert_eq!(
            session.engine.active_track_names_for_test(),
            ["bass_track", "drum_stem"]
        );
        let rendered = session.render_test_block_for_tui(4);
        assert!(rendered.iter().any(|sample| sample.abs() > f32::EPSILON));

        let _ = std::fs::remove_dir_all(directory);
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

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn samples_command_hot_reloads_added_files_for_live_playback() {
        let mut session = ReplSession::new();
        let directory = temp_directory("repl-hot-reload");

        session.eval_line(":tempo 48000").unwrap();
        let _ = session.render_test_block_for_tui(1);
        session
            .eval_line(&format!(":samples {}", directory.display()))
            .unwrap();
        session.eval_line(r#"lead = sample("rim")"#).unwrap();

        let silent = session.render_test_block_for_tui(4);
        assert!(silent.iter().all(|sample| sample.abs() <= f32::EPSILON));

        let start = std::time::Instant::now();
        write_wav(directory.join("rim.wav"), &[0.55, 0.0, 0.0, 0.0]);
        let rendered = wait_for_audible_hot_reload(&mut session, start);
        let expected = 0.55 * edge_envelope(0, 4);

        assert!(start.elapsed() <= std::time::Duration::from_millis(500));
        assert!((rendered[0] - expected).abs() < f32::EPSILON);
        assert!((rendered[1] - expected).abs() < f32::EPSILON);

        std::fs::remove_dir_all(directory).unwrap();
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

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn sample_playback_params_flow_into_live_engine() {
        let mut session = ReplSession::new();
        let directory = temp_directory("repl-sample-params");
        std::fs::write(
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

        std::fs::remove_dir_all(directory).unwrap();
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
            Ok("bound copy = Pattern<Sample>: Pattern<Sample>".to_owned())
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
    fn session_explain_returns_pedal_plan() {
        let mut session = ReplSession::new();
        session
            .eval_line(
                "drivebox = graph { wet = input |> clip(model=silicon_hard); wet |> output }",
            )
            .unwrap();

        let message = session.eval_line(":explain drivebox").unwrap();

        assert!(message.contains("Target Signal Kind:"));
        assert!(message.contains("wet"));
        assert!(message.contains("clip(input, model=silicon_hard)"));
    }

    #[test]
    fn session_explain_returns_sample_pattern_plan() {
        let mut session = ReplSession::new();
        session.eval_line("drums = bd sn").unwrap();

        let plan = session.eval_line(":explain drums").unwrap();

        assert!(plan.contains("Sample Pattern Plan"));
        assert!(plan.contains("drums"));
    }

    #[test]
    fn midi_list_command_returns_available_outputs_or_none() {
        let mut session = ReplSession::new();
        let result = session.eval_line(":midi list");
        match result {
            Ok(message) => assert!(message.starts_with("available MIDI output ports: ")),
            Err(e) => assert!(e.contains("failed to initialize MIDI")),
        }
    }

    #[test]
    fn midi_input_list_command_returns_available_inputs_or_none() {
        let mut session = ReplSession::new();
        let result = session.eval_line(":midi in list");
        match result {
            Ok(message) => assert!(message.starts_with("available MIDI input ports: ")),
            Err(e) => assert!(e.contains("failed to initialize MIDI")),
        }
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
        assert!(
            error.contains("cannot be sent as MIDI notes")
                || error.contains("no MIDI output is connected")
        );
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
        std::fs::create_dir_all(&directory).unwrap();
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

    fn wait_for_audible_hot_reload(
        session: &mut ReplSession,
        start: std::time::Instant,
    ) -> Vec<f32> {
        loop {
            let _ = session.render_test_block_for_tui(session.frames_until_boundary_for_tui());
            let rendered = session.render_test_block_for_tui(4);
            if rendered.iter().any(|sample| sample.abs() > f32::EPSILON) {
                return rendered;
            }
            assert!(
                start.elapsed() <= std::time::Duration::from_millis(500),
                "sample hot reload did not become audible in time"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
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

#[cfg(test)]
mod supercollider_integration_tests {
    use super::*;

    #[test]
    fn export_command_exports_a_bound_pattern_to_supercollider() {
        let mut session = ReplSession::new();
        let path = std::env::temp_dir().join(format!("orpheus-export-{}.scd", 1_234_578));

        session.eval_line("song = bd sn cp sn").unwrap();
        let message = session
            .eval_line(&format!(":export song {} 2", path.display()))
            .unwrap();

        assert!(message.contains("exported `song`"));
        assert!(path.exists());
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("// Orpheus `SuperCollider` Export"));
        assert!(contents.contains("Synth(\\play_sample"));
        assert!(contents.contains("\\bd"));
        assert!(contents.contains("\\sn"));

        let _ = std::fs::remove_file(path);
    }
}

#[test]
fn export_command_exports_number_pattern_to_supercollider() {
    let mut session = ReplSession::new();
    let path = std::env::temp_dir().join("orpheus-export-num-1_234_578.scd");

    session.eval_line("notes = 60 62 64").unwrap();
    let message = session
        .eval_line(&format!(":export notes {} 1", path.display()))
        .unwrap();

    assert!(message.contains("exported `notes`"));
    assert!(path.exists());
    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("// Orpheus `SuperCollider` Export"));
    assert!(contents.contains("Synth(\\default"));
    assert!(contents.contains("60.000.midicps"));

    let _ = std::fs::remove_file(path);
}

fn build_help_table() -> comfy_table::Table {
    let mut table = comfy_table::Table::new();
    table.load_preset(comfy_table::presets::UTF8_BORDERS_ONLY);
    table.set_header(vec![
        comfy_table::Cell::new("Command")
            .fg(comfy_table::Color::White)
            .add_attribute(comfy_table::Attribute::Bold),
        comfy_table::Cell::new("Description")
            .fg(comfy_table::Color::White)
            .add_attribute(comfy_table::Attribute::Bold),
    ]);

    let commands = [
        (":env", "List all available bindings in the environment"),
        (":undo", "Restore the previous compositional session state"),
        (":redo", "Reapply the most recently undone session state"),
        (
            ":explain <binding>",
            "Explain the internal structure of a pattern or pedal",
        ),
        (
            ":stats <binding>",
            "Show event density and statistics for a pattern",
        ),
        (
            ":render <binding> <path> <cycles>",
            "Render a pattern to an audio file (.wav/.flac)",
        ),
        (
            ":export <binding> <path> <cycles>",
            "Export a pattern to various formats (MIDI, SVG, etc.)",
        ),
        (
            ":export stems <binding> <dir> <cycles>",
            "Export individual track stems to a directory",
        ),
        (
            ":export master <path> [cycles] [--no-buses]",
            "Render the full routed mix (voices + buses) to one master WAV",
        ),
        (
            ":roll <binding>",
            "Display an ASCII piano roll of a pattern",
        ),
        (":tempo <bpm>", "Set the global tempo in beats per minute"),
        (
            ":ref_freq <hz>",
            "Set the global reference frequency for tuning",
        ),
        (
            ":samples <dir>",
            "Load additional audio samples from a directory",
        ),
        (
            ":import stems <dir>",
            "Load stem WAVs as sample patterns and routed tracks",
        ),
        (
            ":reload-samples",
            "Reload the most recently loaded sample directory",
        ),
        (":open <path>", "Open and evaluate an external .ode script"),
        (":track ...", "Manage mixer tracks (new, bind, level, mute)"),
        (":bus ...", "Manage mixer buses and effects (new, fx)"),
        (":send ...", "Manage track effect sends to buses"),
        (
            ":mixer",
            "Display the current state of tracks, buses, and sends",
        ),
        (":midi ...", "Manage MIDI inputs and outputs"),
        (":play", "Start the transport clock"),
        (":stop", "Stop the transport clock"),
        (":help", "List available REPL commands and descriptions"),
        (":quit", "Exit the REPL session"),
    ];

    for (cmd, desc) in commands {
        table.add_row(vec![
            comfy_table::Cell::new(cmd).fg(comfy_table::Color::Cyan),
            comfy_table::Cell::new(desc).fg(comfy_table::Color::Green),
        ]);
    }

    table
}
