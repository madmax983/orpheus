use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::Write;
use std::path::Path;

use orpheus_dsp::{OfflineRenderError, SampleBank, SampleTrigger, render_events_to_file_with_bank};
use orpheus_pattern::Event;
use serde_json::{Value as JsonValue, json};

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Errors that can occur during audio rendering or exporting operations.
///
/// This type wraps errors that can happen either when evaluating the pattern
/// ([`EvalError`]) or when the underlying DSP engine renders it to audio
/// ([`OfflineRenderError`]).
///
/// # Examples
///
/// ```
/// use orpheus_lang::{RenderError, EvalError};
///
/// let error = RenderError::Eval(EvalError::new("some error"));
/// assert_eq!(error.to_string(), "some error");
/// ```
#[derive(Debug)]
pub enum RenderError {
    Eval(EvalError),
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
fn sample_trigger_from_event(event: &crate::value::SampleEvent) -> SampleTrigger {
    let mut trigger = SampleTrigger::named(event.sample())
        .with_gain(event.gain())
        .with_pan(event.pan())
        .with_rate(event.rate())
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
    let path = path.as_ref();

    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(
        file,
        "start_num,start_den,start_float,end_num,end_den,end_float,sample,gain,pan,rate,hpf_cutoff_hz,lpf_cutoff_hz"
    )
    .map_err(|e| EvalError::new(e.to_string()))?;

    for event in events {
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
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
    }

    Ok(())
}

/// Exports a sample pattern's evaluated events to a JSON file.
///
/// The JSON document contains a top-level `kind`, `cycle_count`, and `events`
/// array. Each event uses the same timing fields as the CSV exporter plus the
/// sample control fields available at runtime.
///
/// # Examples
///
/// ```
/// use tempfile::NamedTempFile;
/// use orpheus_lang::{ReplMode, eval_module, export_sample_pattern_to_json};
///
/// let env = eval_module("x = bd", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let file = NamedTempFile::new().unwrap();
/// let result = export_sample_pattern_to_json(pattern, file.path(), 2);
///
/// assert!(result.is_ok());
/// ```
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
    let payload = json!({
        "kind": "sample",
        "cycle_count": cycle_count,
        "events": events
            .into_iter()
            .map(|event| sample_event_json(&event))
            .collect::<Vec<_>>(),
    });

    write_json_file(path.as_ref(), &payload)
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
    let path = path.as_ref();

    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(
        file,
        "start_num,start_den,start_float,end_num,end_den,end_float,value"
    )
    .map_err(|e| EvalError::new(e.to_string()))?;

    for event in events {
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
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a JSON file.
///
/// The JSON document contains a top-level `kind`, `cycle_count`, and `events`
/// array. Each event uses the same timing fields as the CSV exporter plus the
/// numeric `value`.
///
/// # Examples
///
/// ```
/// use tempfile::NamedTempFile;
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_json};
///
/// let env = eval_module("x = 1.0", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let file = NamedTempFile::new().unwrap();
/// let result = export_number_pattern_to_json(pattern, file.path(), 2);
///
/// assert!(result.is_ok());
/// ```
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
    let payload = json!({
        "kind": "number",
        "cycle_count": cycle_count,
        "events": events
            .into_iter()
            .map(|event| number_event_json(&event))
            .collect::<Vec<_>>(),
    });

    write_json_file(path.as_ref(), &payload)
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
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, render_sample_pattern_to_wav};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("render_wav.wav");
/// let result = render_sample_pattern_to_wav(pattern, &path, 2);
///
/// assert!(result.is_ok());
/// ```
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

fn sample_event_json(event: &Event<crate::value::SampleEvent>) -> JsonValue {
    json!({
        "start_num": event.part.start().numerator(),
        "start_den": event.part.start().denominator(),
        "start_float": f64::from(event.part.start()),
        "end_num": event.part.end().numerator(),
        "end_den": event.part.end().denominator(),
        "end_float": f64::from(event.part.end()),
        "sample": event.value.sample(),
        "gain": event.value.gain(),
        "pan": event.value.pan(),
        "rate": event.value.rate(),
        "slice_start": event.value.slice_start(),
        "slice_end": event.value.slice_end(),
        "hpf_cutoff_hz": event.value.hpf_cutoff_hz(),
        "lpf_cutoff_hz": event.value.lpf_cutoff_hz(),
    })
}

fn number_event_json(event: &Event<f64>) -> JsonValue {
    json!({
        "start_num": event.part.start().numerator(),
        "start_den": event.part.start().denominator(),
        "start_float": f64::from(event.part.start()),
        "end_num": event.part.end().numerator(),
        "end_den": event.part.end().denominator(),
        "end_float": f64::from(event.part.end()),
        "value": event.value,
    })
}

fn write_json_file(path: &Path, payload: &JsonValue) -> Result<(), EvalError> {
    let file = std::fs::File::create(path).map_err(|error| EvalError::new(error.to_string()))?;
    serde_json::to_writer_pretty(file, payload).map_err(|error| EvalError::new(error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::Value as JsonValue;

    use super::{export_number_pattern_to_json, export_sample_pattern_to_json};
    use crate::{ReplMode, eval_module};

    static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn temp_json_path() -> PathBuf {
        std::env::temp_dir().join(format!("orpheus-export-json-{}.json", unique_temp_suffix()))
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
}
