use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::io::Write;
use std::path::Path;

use orpheus_dsp::{OfflineRenderError, SampleBank, SampleTrigger, render_events_to_file_with_bank};
use orpheus_pattern::Event;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Errors that can occur during audio rendering or exporting operations.
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

    let mut file =
        std::fs::File::create(path.as_ref()).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "{{")
        .and_then(|()| writeln!(file, "  \"kind\": \"sample\","))
        .and_then(|()| writeln!(file, "  \"cycle_count\": {cycle_count},"))
        .and_then(|()| writeln!(file, "  \"events\": ["))
        .map_err(|e| EvalError::new(e.to_string()))?;

    for (i, event) in events.iter().enumerate() {
        if i > 0 {
            writeln!(file, ",").map_err(|e| EvalError::new(e.to_string()))?;
        }
        write!(file, "{}", sample_event_json(event)).map_err(|e| EvalError::new(e.to_string()))?;
    }

    writeln!(file, "\n  ]")
        .and_then(|()| writeln!(file, "}}"))
        .map_err(|e| EvalError::new(e.to_string()))?;

    Ok(())
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

    let mut file =
        std::fs::File::create(path.as_ref()).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "{{")
        .and_then(|()| writeln!(file, "  \"kind\": \"number\","))
        .and_then(|()| writeln!(file, "  \"cycle_count\": {cycle_count},"))
        .and_then(|()| writeln!(file, "  \"events\": ["))
        .map_err(|e| EvalError::new(e.to_string()))?;

    for (i, event) in events.iter().enumerate() {
        if i > 0 {
            writeln!(file, ",").map_err(|e| EvalError::new(e.to_string()))?;
        }
        write!(file, "{}", number_event_json(event)).map_err(|e| EvalError::new(e.to_string()))?;
    }

    writeln!(file, "\n  ]")
        .and_then(|()| writeln!(file, "}}"))
        .map_err(|e| EvalError::new(e.to_string()))?;

    Ok(())
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

pub(crate) fn escape_json_string(s: &str) -> String {
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
}
