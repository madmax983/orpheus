//! The `export` module provides utilities for rendering and saving patterns.
//!
//! This module allows evaluated patterns to be rendered into deterministic offline audio
//! files (WAV), or exported as structured data formats like JSON and CSV. These
//! formats enable interoperability with external tools, data visualization, and DAWs.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::Write;
use std::path::Path;

use orpheus_dsp::{OfflineRenderError, SampleBank, SampleTrigger, render_events_to_file_with_bank};
use orpheus_pattern::Event;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Errors that can occur during audio rendering or exporting operations.
///
/// This error is returned when exporting patterns to audio files (like WAV).
/// It can either stem from runtime evaluation failures (e.g., trying to render a
/// pattern with out-of-bounds parameters) or from the audio engine failing to
/// process and write the PCM data to disk.
///
/// # Causes
///
/// - [`RenderError::Eval`]: The pattern could not be successfully queried across
///   the requested time span due to an [`EvalError`] (e.g., invalid arithmetic
///   on the rational time domain).
/// - [`RenderError::Audio`]: The offline digital signal processing engine failed
///   to write the resulting audio file (e.g., I/O permissions or a corrupted
///   sample bank).
///
/// # Examples
///
/// ```
/// use orpheus_lang::RenderError;
/// use orpheus_lang::EvalError;
///
/// let error = RenderError::Eval(EvalError::new("out of bounds parameter"));
///
/// match error {
///     RenderError::Eval(e) => assert_eq!(e.to_string(), "out of bounds parameter"),
///     RenderError::Audio(_) => unreachable!(),
/// }
/// ```
#[derive(Debug)]
pub enum RenderError {
    /// An error occurred while evaluating the pattern events.
    Eval(EvalError),
    /// An error occurred during the offline digital signal processing or file writing phase.
    Audio(OfflineRenderError),
}

impl Display for RenderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eval(error) => Display::fmt(error, formatter),
            Self::Audio(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for RenderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Eval(error) => Some(error),
            Self::Audio(error) => Some(error),
        }
    }
}

impl From<EvalError> for RenderError {
    fn from(error: EvalError) -> Self {
        Self::Eval(error)
    }
}

impl From<OfflineRenderError> for RenderError {
    fn from(error: OfflineRenderError) -> Self {
        Self::Audio(error)
    }
}

/// Helper function to convert a `SampleEvent` from the evaluation phase into a
/// `SampleTrigger` for the DSP rendering phase.
pub(crate) fn sample_trigger_from_event(event: &crate::value::SampleEvent) -> SampleTrigger {
    let mut trigger = SampleTrigger::named(event.sample())
        .with_gain(event.gain())
        .with_pan(event.pan())
        .with_rate(event.rate())
        .with_delay_mix(event.delay_mix())
        .with_delay_time(event.delay_time())
        .with_delay_feedback(event.delay_feedback())
        .with_reverb_mix(event.reverb_mix())
        .with_reverb_room(event.reverb_room())
        .with_reverb_damp(event.reverb_damp())
        .with_chorus_mix(event.chorus_mix())
        .with_chorus_depth(event.chorus_depth())
        .with_chorus_rate(event.chorus_rate())
        .with_compressor_mix(event.compressor_mix())
        .with_compressor_threshold(event.compressor_threshold())
        .with_compressor_ratio(event.compressor_ratio())
        .with_resonance(event.resonance())
        .with_drive(event.drive())
        .with_pulse_width(event.pulse_width())
        .with_slice(event.slice_start(), event.slice_end());
    if let Some(cutoff) = event.hpf_cutoff_hz() {
        trigger = trigger.with_hpf_cutoff_hz(cutoff);
    }
    if let Some(cutoff) = event.lpf_cutoff_hz() {
        trigger = trigger.with_lpf_cutoff_hz(cutoff);
    }
    trigger
}

/// Renders a sample pattern to a deterministic stereo audio file.
///
/// This uses the builtin sample bank.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, render_sample_pattern_to_file};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("render.wav");
/// render_sample_pattern_to_file(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`RenderError`] if pattern querying fails or if the offline audio
/// renderer cannot write the target file.
pub fn render_sample_pattern_to_file(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), RenderError> {
    let sample_bank = SampleBank::load_builtin();
    render_sample_pattern_to_file_with_bank(pattern, path, cycle_count, &sample_bank)
}

fn export_pattern_events_to_csv<T, F>(
    events: &[Event<T>],
    path: impl AsRef<Path>,
    header: &str,
    mut write_event: F,
) -> Result<(), EvalError>
where
    F: FnMut(&mut std::fs::File, &Event<T>) -> Result<(), EvalError>,
{
    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;
    writeln!(file, "{header}")?;

    for event in events {
        write_event(&mut file, event)?;
    }

    Ok(())
}

fn export_pattern_events_to_md<T, F>(
    events: &[Event<T>],
    path: impl AsRef<Path>,
    header: &str,
    mut write_event: F,
) -> Result<(), EvalError>
where
    F: FnMut(&mut std::fs::File, &Event<T>) -> Result<(), EvalError>,
{
    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;
    writeln!(file, "{header}")?;
    // Add Markdown table separator
    let separators = header
        .split('|')
        .map(|s| if s.is_empty() { "" } else { "---" })
        .collect::<Vec<_>>()
        .join("|");
    writeln!(file, "{separators}")?;

    for event in events {
        write_event(&mut file, event)?;
    }

    Ok(())
}

/// Exports a sample pattern's evaluated events to a Markdown file.
///
/// The Markdown file will contain a table with columns for `start`,
/// `end`, `sample`, `gain`, `pan`, `rate`, `hpf`, and `lpf`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_sample_pattern_to_md};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.md");
/// export_sample_pattern_to_md(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if the cycle count is 0, if pattern querying fails, or if the file cannot be written.
pub fn export_sample_pattern_to_md(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;

    export_pattern_events_to_md(
        &events,
        path,
        "| start | end | sample | gain | pan | rate | hpf | lpf |",
        |file, event| {
            let start_float = f64::from(event.part.start());
            let end_float = f64::from(event.part.end());
            let hpf = event
                .value
                .hpf_cutoff_hz()
                .map_or_else(|| "-".to_owned(), |v| format!("{v:.2}"));
            let lpf = event
                .value
                .lpf_cutoff_hz()
                .map_or_else(|| "-".to_owned(), |v| format!("{v:.2}"));
            writeln!(
                file,
                "| {:.3} | {:.3} | {} | {:.2} | {:.2} | {:.2} | {} | {} |",
                start_float,
                end_float,
                event.value.sample(),
                event.value.gain(),
                event.value.pan(),
                event.value.rate(),
                hpf,
                lpf
            )?;
            Ok(())
        },
    )
}

/// Exports a sample pattern's evaluated events to a CSV file.
///
/// The CSV file will contain columns for `start_num`, `start_den`, `start_float`,
/// `end_num`, `end_den`, `end_float`, `sample`, `gain`, `pan`, `rate`,
/// `hpf_cutoff_hz`, and `lpf_cutoff_hz`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_sample_pattern_to_csv};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.csv");
/// export_sample_pattern_to_csv(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if the cycle count is 0, if pattern querying fails, or if the file cannot be written.
pub fn export_sample_pattern_to_csv(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;

    export_pattern_events_to_csv(
        &events,
        path,
        "start_num,start_den,start_float,end_num,end_den,end_float,sample,gain,pan,rate,hpf_cutoff_hz,lpf_cutoff_hz",
        |file, event| {
            let start_float = f64::from(event.part.start());
            let end_float = f64::from(event.part.end());
            let hpf = event
                .value
                .hpf_cutoff_hz()
                .map_or_else(String::new, |v| format!("{v:.6}"));
            let lpf = event
                .value
                .lpf_cutoff_hz()
                .map_or_else(String::new, |v| format!("{v:.6}"));
            writeln!(
                file,
                "{},{},{:.6},{},{},{:.6},{},{:.6},{:.6},{:.6},{},{}",
                event.part.start().numerator(),
                event.part.start().denominator(),
                start_float,
                event.part.end().numerator(),
                event.part.end().denominator(),
                end_float,
                event.value.sample(),
                event.value.gain(),
                event.value.pan(),
                event.value.rate(),
                hpf,
                lpf
            )?;
            Ok(())
        },
    )
}

fn export_pattern_events_to_json<T, F>(
    events: &[Event<T>],
    path: impl AsRef<Path>,
    kind: &str,
    cycle_count: u64,
    mut event_to_json: F,
) -> Result<(), EvalError>
where
    F: FnMut(&Event<T>) -> String,
{
    let mut file = std::fs::File::create(path.as_ref())?;

    writeln!(file, "{{")
        .and_then(|()| writeln!(file, "  \"kind\": \"{kind}\","))
        .and_then(|()| writeln!(file, "  \"cycle_count\": {cycle_count},"))
        .and_then(|()| writeln!(file, "  \"events\": ["))?;

    for (i, event) in events.iter().enumerate() {
        if i > 0 {
            writeln!(file, ",")?;
        }
        write!(file, "{}", event_to_json(event))?;
    }

    writeln!(file, "\n  ]").and_then(|()| writeln!(file, "}}"))?;

    Ok(())
}

/// Exports a sample pattern's evaluated events to a JSON file.
///
/// The JSON document contains a top-level `kind`, `cycle_count`, and `events`
/// array. Each event uses the same timing fields as the CSV exporter plus the
/// sample control fields available at runtime.
///
/// # Errors
///
/// Returns [`EvalError`] if the cycle count is 0, if pattern querying fails, or
/// if the file cannot be written.
pub fn export_sample_pattern_to_json(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;

    export_pattern_events_to_json(&events, path, "sample", cycle_count, sample_event_json)
}

/// Exports a number pattern's evaluated events to a Markdown file.
///
/// The Markdown file will contain a table with columns for `start`,
/// `end`, and `value`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_md};
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_number_pattern.md");
/// export_number_pattern_to_md(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if the cycle count is 0, if pattern querying fails, or if the file cannot be written.
pub fn export_number_pattern_to_md(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;

    export_pattern_events_to_md(&events, path, "| start | end | value |", |file, event| {
        let start_float = f64::from(event.part.start());
        let end_float = f64::from(event.part.end());
        writeln!(
            file,
            "| {:.3} | {:.3} | {:.3} |",
            start_float, end_float, event.value
        )?;
        Ok(())
    })
}

/// Exports a number pattern's evaluated events to a CSV file.
///
/// The CSV file will contain columns for `start_num`, `start_den`, `start_float`,
/// `end_num`, `end_den`, `end_float`, and `value`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_csv};
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_number_pattern.csv");
/// export_number_pattern_to_csv(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if the cycle count is 0, if pattern querying fails, or if the file cannot be written.
pub fn export_number_pattern_to_csv(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;

    export_pattern_events_to_csv(
        &events,
        path,
        "start_num,start_den,start_float,end_num,end_den,end_float,value",
        |file, event| {
            let start_float = f64::from(event.part.start());
            let end_float = f64::from(event.part.end());
            writeln!(
                file,
                "{},{},{:.6},{},{},{:.6},{:.6}",
                event.part.start().numerator(),
                event.part.start().denominator(),
                start_float,
                event.part.end().numerator(),
                event.part.end().denominator(),
                end_float,
                event.value
            )?;
            Ok(())
        },
    )
}

/// Exports a number pattern's evaluated events to a JSON file.
///
/// The JSON document contains a top-level `kind`, `cycle_count`, and `events`
/// array. Each event uses the same timing fields as the CSV exporter plus the
/// numeric `value`.
///
/// # Errors
///
/// Returns [`EvalError`] if the cycle count is 0, if pattern querying fails, or
/// if the file cannot be written.
pub fn export_number_pattern_to_json(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;

    export_pattern_events_to_json(&events, path, "number", cycle_count, number_event_json)
}

/// Renders a sample pattern to a deterministic stereo audio file using the
/// supplied sample bank overrides.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, render_sample_pattern_to_file_with_bank};
/// use orpheus_dsp::SampleBank;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let sample_bank = SampleBank::load_builtin();
/// let path = std::env::temp_dir().join("render_with_bank.wav");
/// render_sample_pattern_to_file_with_bank(pattern, &path, 2, &sample_bank).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`RenderError`] if pattern querying fails, if the offline audio
/// renderer cannot write the target file, or if `cycle_count` is 0.
pub fn render_sample_pattern_to_file_with_bank(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
    sample_bank: &SampleBank,
) -> Result<(), RenderError> {
    if cycle_count == 0 {
        return Err(EvalError::new("rendering requires at least one cycle").into());
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;
    let rendered_events = events
        .into_iter()
        .map(|event| Event {
            whole: event.whole,
            part: event.part,
            value: sample_trigger_from_event(&event.value),
        })
        .collect::<Vec<_>>();

    render_events_to_file_with_bank(path, &rendered_events, cycle_count, sample_bank)?;

    Ok(())
}

/// Renders a sample pattern directly to a WAV file.
///
/// # Errors
///
/// Returns [`RenderError`] if pattern querying fails, if the offline audio
/// renderer cannot write the buffer, or if `cycle_count` is 0.
pub fn render_sample_pattern_to_wav(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), RenderError> {
    render_sample_pattern_to_file(pattern, path, cycle_count)
}

use std::fmt::Write as _;

/// Escapes a string for safe inclusion within a JSON payload.
///
/// Converts double quotes, backslashes, and control characters into their corresponding
/// JSON escape sequences (e.g., `\"`, `\\`, `\n`). This ensures that dynamically generated
/// text (like sample names) won't break the JSON structure during export operations.
///
/// ## Examples
///
/// ```
/// use orpheus_lang::export::escape_json_string;
///
/// let raw = "hello \"world\"\nfrom \\rust\\";
/// let escaped = escape_json_string(raw);
///
/// assert_eq!(escaped, "hello \\\"world\\\"\\nfrom \\\\rust\\\\");
/// ```
pub fn escape_json_string(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match c {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\x08' => escaped.push_str("\\b"),
            '\x0c' => escaped.push_str("\\f"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(escaped, "\\u{:04x}", c as u32);
            }
            c => escaped.push(c),
        }
    }
    escaped
}

fn sample_event_json(event: &Event<crate::value::SampleEvent>) -> String {
    let mut s = String::new();
    s.push_str("    {\n");
    let _ = writeln!(
        s,
        "      \"start_num\": {},",
        event.part.start().numerator()
    );
    let _ = writeln!(
        s,
        "      \"start_den\": {},",
        event.part.start().denominator()
    );
    let _ = writeln!(
        s,
        "      \"start_float\": {:.6},",
        f64::from(event.part.start())
    );
    let _ = writeln!(s, "      \"end_num\": {},", event.part.end().numerator());
    let _ = writeln!(s, "      \"end_den\": {},", event.part.end().denominator());
    let _ = writeln!(
        s,
        "      \"end_float\": {:.6},",
        f64::from(event.part.end())
    );
    let _ = writeln!(
        s,
        "      \"sample\": \"{}\",",
        escape_json_string(event.value.sample())
    );
    let _ = writeln!(s, "      \"gain\": {:.6},", event.value.gain());
    let _ = writeln!(s, "      \"pan\": {:.6},", event.value.pan());
    let _ = writeln!(s, "      \"rate\": {:.6},", event.value.rate());
    let _ = writeln!(s, "      \"delay_mix\": {:.6},", event.value.delay_mix());
    let _ = writeln!(s, "      \"delay_time\": {:.6},", event.value.delay_time());
    let _ = writeln!(
        s,
        "      \"delay_feedback\": {:.6},",
        event.value.delay_feedback()
    );
    let _ = writeln!(s, "      \"reverb_mix\": {:.6},", event.value.reverb_mix());
    let _ = writeln!(
        s,
        "      \"reverb_room\": {:.6},",
        event.value.reverb_room()
    );
    let _ = writeln!(
        s,
        "      \"reverb_damp\": {:.6},",
        event.value.reverb_damp()
    );
    let _ = writeln!(s, "      \"chorus_mix\": {:.6},", event.value.chorus_mix());
    let _ = writeln!(
        s,
        "      \"chorus_depth\": {:.6},",
        event.value.chorus_depth()
    );
    let _ = writeln!(
        s,
        "      \"chorus_rate\": {:.6},",
        event.value.chorus_rate()
    );
    let _ = writeln!(
        s,
        "      \"compressor_mix\": {:.6},",
        event.value.compressor_mix()
    );
    let _ = writeln!(
        s,
        "      \"compressor_threshold\": {:.6},",
        event.value.compressor_threshold()
    );
    let _ = writeln!(
        s,
        "      \"compressor_ratio\": {:.6},",
        event.value.compressor_ratio()
    );
    let _ = writeln!(s, "      \"resonance\": {:.6},", event.value.resonance());
    let _ = writeln!(s, "      \"drive\": {:.6},", event.value.drive());
    let _ = writeln!(
        s,
        "      \"pulse_width\": {:.6},",
        event.value.pulse_width()
    );
    let _ = writeln!(
        s,
        "      \"slice_start\": {:.6},",
        event.value.slice_start()
    );
    let _ = writeln!(s, "      \"slice_end\": {:.6},", event.value.slice_end());

    if let Some(hpf) = event.value.hpf_cutoff_hz() {
        let _ = writeln!(s, "      \"hpf_cutoff_hz\": {hpf:.6},");
    } else {
        s.push_str("      \"hpf_cutoff_hz\": null,\n");
    }

    if let Some(lpf) = event.value.lpf_cutoff_hz() {
        let _ = writeln!(s, "      \"lpf_cutoff_hz\": {lpf:.6}");
    } else {
        s.push_str("      \"lpf_cutoff_hz\": null\n");
    }

    s.push_str("    }");
    s
}

fn number_event_json(event: &Event<f64>) -> String {
    let mut s = String::new();
    s.push_str("    {\n");
    let _ = writeln!(
        s,
        "      \"start_num\": {},",
        event.part.start().numerator()
    );
    let _ = writeln!(
        s,
        "      \"start_den\": {},",
        event.part.start().denominator()
    );
    let _ = writeln!(
        s,
        "      \"start_float\": {:.6},",
        f64::from(event.part.start())
    );
    let _ = writeln!(s, "      \"end_num\": {},", event.part.end().numerator());
    let _ = writeln!(s, "      \"end_den\": {},", event.part.end().denominator());
    let _ = writeln!(
        s,
        "      \"end_float\": {:.6},",
        f64::from(event.part.end())
    );
    let _ = writeln!(s, "      \"value\": {:.6}", event.value);
    s.push_str("    }");
    s
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::Value as JsonValue;

    use super::{
        export_number_pattern_to_json, export_number_pattern_to_md, export_sample_pattern_to_json,
        export_sample_pattern_to_md,
    };
    use crate::{ReplMode, eval_module};

    static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn temp_json_path() -> PathBuf {
        std::env::temp_dir().join(format!("orpheus-export-json-{}.json", unique_temp_suffix()))
    }

    fn temp_md_path() -> PathBuf {
        std::env::temp_dir().join(format!("orpheus-export-md-{}.md", unique_temp_suffix()))
    }

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn unique_temp_suffix() -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        format!("{timestamp}-{counter}")
    }

    fn assert_json_fixture_matches(path: &PathBuf, fixture_name: &str) {
        let actual_contents = fs::read_to_string(path).unwrap();
        let expected_contents = fs::read_to_string(fixture(fixture_name)).unwrap();
        let actual_json: JsonValue = serde_json::from_str(&actual_contents).unwrap();
        let expected_json: JsonValue = serde_json::from_str(&expected_contents).unwrap();

        assert_eq!(actual_json, expected_json);
    }

    #[test]
    fn render_error_from_eval_error() {
        let eval_err = super::EvalError::new("an evaluation error");
        let err: super::RenderError = eval_err.into();
        assert_eq!(err.to_string(), "an evaluation error");
    }

    #[test]
    fn render_error_from_offline_render_error() {
        use orpheus_dsp::OfflineRenderError;
        let dsp_err = OfflineRenderError::InvalidCycleCount;
        let err: super::RenderError = dsp_err.into();
        assert_eq!(
            err.to_string(),
            "offline rendering requires at least one cycle"
        );
    }

    #[test]
    fn sample_json_export_preserves_parameterized_binding_output() {
        let module = eval_module(
            "swing amt pat = pat |> shift(amt)\n\
             groove = bd sn |> swing(0.25)",
            ReplMode::Loose,
        )
        .unwrap();
        let groove = module.get("groove").unwrap().as_sample_pattern().unwrap();
        let path = temp_json_path();

        export_sample_pattern_to_json(groove, &path, 1).unwrap();
        assert_json_fixture_matches(&path, "parameterized_groove_export.json");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn number_json_export_preserves_parameterized_binding_output() {
        let module = eval_module(
            "id pat = pat\n\
             cutoff = id(400 800)",
            ReplMode::Loose,
        )
        .unwrap();
        let cutoff = module.get("cutoff").unwrap().as_number_pattern().unwrap();
        let path = temp_json_path();

        export_number_pattern_to_json(cutoff, &path, 1).unwrap();
        assert_json_fixture_matches(&path, "parameterized_cutoff_export.json");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn euclid_export_preserves_masked_sample_output() {
        let module = eval_module(
            "drums = mask(euclid(3, 8), bd sn cp hh bd sn cp hh)",
            ReplMode::Loose,
        )
        .unwrap();
        let drums = module.get("drums").unwrap().as_sample_pattern().unwrap();
        let path = temp_json_path();

        export_sample_pattern_to_json(drums, &path, 1).unwrap();
        assert_json_fixture_matches(&path, "euclid_masked_drums_export.json");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn pitch_class_set_export_preserves_parameterized_melody_output() {
        let module = eval_module(
            "walk set pat = degrees(set, pat) |> transpose(60)\n\
             hirajoshi = pitch_class_set(0 2 3 7 8)\n\
             melody = walk(hirajoshi)(0 1 2 4 5)",
            ReplMode::Loose,
        )
        .unwrap();
        let melody = module.get("melody").unwrap().as_number_pattern().unwrap();
        let path = temp_json_path();

        export_number_pattern_to_json(melody, &path, 1).unwrap();
        assert_json_fixture_matches(&path, "pitch_class_set_melody_export.json");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn named_pitch_export_preserves_absolute_melody_output() {
        let module = eval_module("melody = c4 ef4 g4 bf4", ReplMode::Loose).unwrap();
        let melody = module.get("melody").unwrap().as_number_pattern().unwrap();
        let path = temp_json_path();

        export_number_pattern_to_json(melody, &path, 1).unwrap();
        assert_json_fixture_matches(&path, "named_pitch_melody_export.json");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn chord_export_preserves_progression_output() {
        let module = eval_module("pads = chord(c4 e4, 0 7)", ReplMode::Loose).unwrap();
        let pads = module.get("pads").unwrap().as_number_pattern().unwrap();
        let path = temp_json_path();

        export_number_pattern_to_json(pads, &path, 1).unwrap();
        assert_json_fixture_matches(&path, "chord_progression_export.json");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn invert_export_preserves_voiced_progression_output() {
        let module = eval_module("pads = invert(1, chord(c4 e4, 0 7))", ReplMode::Loose).unwrap();
        let pads = module.get("pads").unwrap().as_number_pattern().unwrap();
        let path = temp_json_path();

        export_number_pattern_to_json(pads, &path, 1).unwrap();
        assert_json_fixture_matches(&path, "invert_progression_export.json");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn drop_export_preserves_dropped_progression_output() {
        let module = eval_module("pads = drop(2, chord(c4 e4, 0 7 10))", ReplMode::Loose).unwrap();
        let pads = module.get("pads").unwrap().as_number_pattern().unwrap();
        let path = temp_json_path();

        export_number_pattern_to_json(pads, &path, 1).unwrap();
        assert_json_fixture_matches(&path, "drop_progression_export.json");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn strum_export_preserves_partitioned_progression_output() {
        let module = eval_module("pads = strum(chord(c4 e4, 0 7))", ReplMode::Loose).unwrap();
        let pads = module.get("pads").unwrap().as_number_pattern().unwrap();
        let path = temp_json_path();

        export_number_pattern_to_json(pads, &path, 1).unwrap();
        assert_json_fixture_matches(&path, "strum_progression_export.json");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn arp_export_preserves_wrapped_progression_output() {
        let module = eval_module("lead = arp(5, up, chord(c4, 0 4 7))", ReplMode::Loose).unwrap();
        let lead = module.get("lead").unwrap().as_number_pattern().unwrap();
        let path = temp_json_path();

        export_number_pattern_to_json(lead, &path, 1).unwrap();
        assert_json_fixture_matches(&path, "arp_progression_export.json");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn roll_export_preserves_retriggered_sample_output() {
        let module = eval_module("buzz = roll(4, sn)", ReplMode::Loose).unwrap();
        let buzz = module.get("buzz").unwrap().as_sample_pattern().unwrap();
        let path = temp_json_path();

        export_sample_pattern_to_json(buzz, &path, 1).unwrap();
        assert_json_fixture_matches(&path, "roll_progression_export.json");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_csv(pat, "test.csv", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
        assert_eq!(
            export_sample_pattern_to_json(pat, "test.json", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
        assert_eq!(
            super::render_sample_pattern_to_file_with_bank(
                pat,
                "test.wav",
                0,
                &orpheus_dsp::SampleBank::default()
            )
            .unwrap_err()
            .to_string(),
            "rendering requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_csv(pat, "test.csv", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
        assert_eq!(
            super::export_number_pattern_to_json(pat, "test.json", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }

    #[test]
    fn escape_json_string_handles_special_characters() {
        assert_eq!(super::escape_json_string("normal"), "normal");
        assert_eq!(super::escape_json_string("a\"b"), "a\\\"b");
        assert_eq!(super::escape_json_string("a\\b"), "a\\\\b");
        assert_eq!(super::escape_json_string("a\x08b"), "a\\bb");
        assert_eq!(super::escape_json_string("a\x0cb"), "a\\fb");
        assert_eq!(super::escape_json_string("a\nb"), "a\\nb");
        assert_eq!(super::escape_json_string("a\rb"), "a\\rb");
        assert_eq!(super::escape_json_string("a\tb"), "a\\tb");
        assert_eq!(super::escape_json_string("a\x01b"), "a\\u0001b");
    }

    #[test]
    fn json_export_handles_hpf_lpf_cutoff() {
        let module = eval_module("pat = bd |> lpf(400) |> hpf(100)", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();
        let path = temp_json_path();

        export_sample_pattern_to_json(pat, &path, 1).unwrap();

        let actual_contents = fs::read_to_string(&path).unwrap();
        assert!(actual_contents.contains("\"lpf_cutoff_hz\": 400.000000"));
        assert!(actual_contents.contains("\"hpf_cutoff_hz\": 100.000000"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn md_export_formats_sample_pattern() {
        let module = eval_module("pat = bd |> lpf(400) |> hpf(100)", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();
        let path = temp_md_path();

        export_sample_pattern_to_md(pat, &path, 1).unwrap();

        let actual_contents = fs::read_to_string(&path).unwrap();
        assert!(
            actual_contents.contains("| start | end | sample | gain | pan | rate | hpf | lpf |")
        );
        assert!(actual_contents.contains("|---|---|---|---|---|---|---|---|"));
        assert!(
            actual_contents
                .contains("| 0.000 | 1.000 | bd | 1.00 | 0.00 | 1.00 | 100.00 | 400.00 |")
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn md_export_formats_number_pattern() {
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();
        let path = temp_md_path();

        export_number_pattern_to_md(pat, &path, 1).unwrap();

        let actual_contents = fs::read_to_string(&path).unwrap();
        assert!(actual_contents.contains("| start | end | value |"));
        assert!(actual_contents.contains("|---|---|---|"));
        assert!(actual_contents.contains("| 0.000 | 0.500 | 1.000 |"));
        assert!(actual_contents.contains("| 0.500 | 1.000 | 2.000 |"));

        let _ = fs::remove_file(path);
    }
}
