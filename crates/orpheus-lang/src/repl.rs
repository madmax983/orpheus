use crossterm::style::Stylize;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use orpheus_dsp::{
    EngineCommand, EngineHandle, PatternUpdate, SampleBank, SampleTrigger, TransportSnapshot,
    load_sample_bank_from_directory,
};

use crate::eval::{eval_into_bindings, render_sample_pattern_to_file_with_bank};
use crate::loader::load_file_runtime_strict;
use crate::types::infer_into_bindings;
use crate::{ReplMode, Type, Value};

/// Runs the phase-one Orpheus REPL over standard input and output.
///
/// Blank lines are ignored. `:quit` exits the session.
///
/// # Errors
///
/// Returns any terminal I/O failure encountered while reading input or
/// writing REPL output.
pub fn run_stdio() -> io::Result<()> {
    run_stdio_with_engine(EngineHandle::stub())
}

/// Runs the phase-one Orpheus REPL with the provided audio engine handle.
///
/// Blank lines are ignored. `:quit` exits the session.
///
/// # Errors
///
/// Returns any terminal I/O failure encountered while reading input or
/// writing REPL output.
pub fn run_stdio_with_engine(engine: EngineHandle) -> io::Result<()> {
    run_stdio_with_engine_and_path(engine, None, None)
}

/// Runs the phase-one Orpheus REPL with an optional startup `.ode` preload.
///
/// # Errors
///
/// Returns startup file load failures or terminal I/O failures encountered
/// while the REPL is active.
pub fn run_stdio_with_engine_and_path(
    engine: EngineHandle,
    startup_path: Option<&Path>,
    warning: Option<String>,
) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut session = ReplSession::with_engine(engine);

    let mut stdout = stdout.lock();
    let mut stderr = stderr.lock();

    if let Some(msg) = warning {
        writeln!(stderr, "{}", format!("⚠️ {msg}").yellow().bold())?;
    }

    if let Some(path) = startup_path {
        match session.open_file(path) {
            Ok(msg) => writeln!(stdout, "{}", msg.green())?,
            Err(msg) => writeln!(stderr, "{}", format!("⚠️ {msg}").yellow().bold())?,
        }
    }

    run_with_handles(stdin.lock(), stdout, stderr, &mut session)
}

fn run_with_handles<R, W, E>(
    mut reader: R,
    mut stdout: W,
    mut stderr: E,
    session: &mut ReplSession,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
    E: Write,
{
    let mut line = String::new();
    loop {
        write!(stdout, "{}", "> ".dark_grey())?;
        stdout.flush()?;
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == ":quit" {
            break;
        }

        match session.eval_line(trimmed) {
            Ok(message) => writeln!(stdout, "{}", format!("✓ {message}").green())?,
            Err(message) => writeln!(stderr, "{}", format!("✗ {message}").yellow().bold())?,
        }
    }

    Ok(())
}

pub(crate) struct ReplSession {
    mode: ReplMode,
    engine: EngineHandle,
    sample_bank: SampleBank,
    sample_directory: Option<PathBuf>,
    bindings: BTreeMap<String, Value>,
    type_bindings: BTreeMap<String, Type>,
    pattern_display: RefCell<PatternDisplayState>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PatternDisplayState {
    active_pattern_name: Option<String>,
    pending_pattern_name: Option<String>,
    pending_enqueued_after_publish: Option<u64>,
    last_loaded_pattern_name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TransportView {
    snapshot: TransportSnapshot,
    active_pattern_name: Option<String>,
    pending_pattern_name: Option<String>,
}

impl TransportView {
    #[must_use]
    pub const fn snapshot(&self) -> &TransportSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub fn active_pattern_name(&self) -> Option<&str> {
        self.active_pattern_name.as_deref()
    }

    #[must_use]
    pub fn pending_pattern_name(&self) -> Option<&str> {
        self.pending_pattern_name.as_deref()
    }
}

impl ReplSession {
    #[cfg(test)]
    fn new() -> Self {
        Self::with_engine(EngineHandle::stub())
    }

    pub(crate) fn with_engine(engine: EngineHandle) -> Self {
        Self {
            mode: ReplMode::Loose,
            engine,
            sample_bank: SampleBank::load_builtin(),
            sample_directory: None,
            bindings: BTreeMap::new(),
            type_bindings: BTreeMap::new(),
            pattern_display: RefCell::new(PatternDisplayState::default()),
        }
    }

    pub(crate) fn eval_line(&mut self, source: &str) -> Result<String, String> {
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
            "export" => {
                if args.is_empty() {
                    Err(export_usage().to_owned())
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
            .map_err(|error| error.to_string())?;
        Ok(format!(
            "rendered `{binding_name}` to `{path}` ({cycles} cycle(s))"
        ))
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

        match value {
            Value::SamplePattern(pattern) => {
                if std::path::Path::new(&path)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
                {
                    crate::svg::export_sample_pattern_to_svg(pattern, &path, cycles)
                        .map_err(|error| error.to_string())?;
                } else {
                    crate::eval::export_sample_pattern_to_csv(pattern, &path, cycles)
                        .map_err(|error| error.to_string())?;
                }
            }
            Value::NumberPattern(pattern) => {
                if std::path::Path::new(&path)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
                {
                    return Err("number patterns cannot be exported to SVG".to_string());
                }
                crate::eval::export_number_pattern_to_csv(pattern, &path, cycles)
                    .map_err(|error| error.to_string())?;
            }
            Value::Function(_) | Value::String(_) => {
                return Err(format!(
                    "binding `{binding_name}` is a {} and cannot be exported",
                    value.kind_name()
                ));
            }
        }

        Ok(format!(
            "exported `{binding_name}` to `{path}` ({cycles} cycle(s))"
        ))
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

    pub(crate) fn open_file(&mut self, path: impl AsRef<Path>) -> Result<String, String> {
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

    fn push_pattern_update(&mut self, name: &str, value: &Value) -> Result<(), String> {
        if let Value::SamplePattern(pattern) = value {
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
                        value: {
                            let mut trigger = SampleTrigger::named(event.value.sample())
                                .with_gain(event.value.gain())
                                .with_pan(event.value.pan())
                                .with_rate(event.value.rate())
                                .with_slice(event.value.slice_start(), event.value.slice_end());
                            if let Some(cutoff_hz) = event.value.hpf_cutoff_hz() {
                                trigger = trigger.with_hpf_cutoff_hz(cutoff_hz);
                            }
                            if let Some(cutoff_hz) = event.value.lpf_cutoff_hz() {
                                trigger = trigger.with_lpf_cutoff_hz(cutoff_hz);
                            }
                            trigger
                        },
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

    pub(crate) fn binding_summaries(&self) -> Vec<String> {
        self.type_bindings
            .iter()
            .map(|(name, ty)| format!("{name}: {ty}"))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn last_loaded_pattern_name(&self) -> Option<String> {
        self.pattern_display
            .borrow()
            .last_loaded_pattern_name
            .clone()
    }

    pub(crate) fn transport_snapshot(&self) -> TransportSnapshot {
        self.transport_view().snapshot
    }

    pub(crate) fn transport_view(&self) -> TransportView {
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

    #[cfg(test)]
    pub(crate) fn render_test_block_for_tui(&mut self, frames: u64) -> Vec<f32> {
        self.engine.render_test_block(frames)
    }

    #[cfg(test)]
    pub(crate) fn frames_until_boundary_for_tui(&self) -> u64 {
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
    "usage: :export <binding> <path> [cycles]"
}

const fn tempo_usage() -> &'static str {
    "usage: :tempo <bpm>"
}

const fn samples_usage() -> &'static str {
    "usage: :samples <directory>"
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
    fn export_command_refuses_to_export_number_pattern_to_svg() {
        let mut session = ReplSession::new();
        let path = temp_svg_path();

        session.eval_line("notes = 1 2 3").unwrap();
        let err = session
            .eval_line(&format!(":export notes {} 1", path.display()))
            .unwrap_err();

        assert!(err.contains("number patterns cannot be exported to SVG"));
        assert!(!path.exists());
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
