use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to a JSON file.
///
/// The JSON file will contain an array of objects representing the events,
/// with keys for `start_num`, `start_den`, `start_float`,
/// `end_num`, `end_den`, `end_float`, `sample`, `gain`, `pan`, `rate`,
/// `hpf_cutoff_hz`, and `lpf_cutoff_hz`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::json::export_sample_pattern_to_json;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.json");
/// export_sample_pattern_to_json(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if the cycle count is 0, if pattern querying fails, or if the file cannot be written.
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
    let path = path.as_ref();

    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(file, "[").map_err(|e| EvalError::new(e.to_string()))?;

    for (i, event) in events.iter().enumerate() {
        let start_float = f64::from(event.part.start());
        let end_float = f64::from(event.part.end());
        let hpf = event
            .value
            .hpf_cutoff_hz()
            .map_or_else(|| "null".to_owned(), |v| format!("{v:.6}"));
        let lpf = event
            .value
            .lpf_cutoff_hz()
            .map_or_else(|| "null".to_owned(), |v| format!("{v:.6}"));

        writeln!(
            file,
            "  {{
    \"start_num\": {},
    \"start_den\": {},
    \"start_float\": {:.6},
    \"end_num\": {},
    \"end_den\": {},
    \"end_float\": {:.6},
    \"sample\": \"{}\",
    \"gain\": {:.6},
    \"pan\": {:.6},
    \"rate\": {:.6},
    \"hpf_cutoff_hz\": {},
    \"lpf_cutoff_hz\": {}
  }}{}",
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
            lpf,
            if i < events.len() - 1 { "," } else { "" }
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
    }

    writeln!(file, "]").map_err(|e| EvalError::new(e.to_string()))?;

    Ok(())
}

/// Exports a number pattern's evaluated events to a JSON file.
///
/// The JSON file will contain an array of objects representing the events,
/// with keys for `start_num`, `start_den`, `start_float`,
/// `end_num`, `end_den`, `end_float`, and `value`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::json::export_number_pattern_to_json;
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_number_pattern.json");
/// export_number_pattern_to_json(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if the cycle count is 0, if pattern querying fails, or if the file cannot be written.
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
    let path = path.as_ref();

    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(file, "[").map_err(|e| EvalError::new(e.to_string()))?;

    for (i, event) in events.iter().enumerate() {
        let start_float = f64::from(event.part.start());
        let end_float = f64::from(event.part.end());
        writeln!(
            file,
            "  {{
    \"start_num\": {},
    \"start_den\": {},
    \"start_float\": {:.6},
    \"end_num\": {},
    \"end_den\": {},
    \"end_float\": {:.6},
    \"value\": {:.6}
  }}{}",
            event.part.start().numerator(),
            event.part.start().denominator(),
            start_float,
            event.part.end().numerator(),
            event.part.end().denominator(),
            end_float,
            event.value,
            if i < events.len() - 1 { "," } else { "" }
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
    }

    writeln!(file, "]").map_err(|e| EvalError::new(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};
    use std::fs;

    #[test]
    fn test_sample_pattern_json_export() {
        let env = eval_module("x = bd", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
        let path = std::env::temp_dir().join("test_sample.json");

        export_sample_pattern_to_json(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with('['));
        assert!(content.ends_with("]\n"));
        assert!(content.contains("\"sample\": \"bd\""));
        assert!(content.contains("\"start_num\": 0"));
        assert!(content.contains("\"end_num\": 1"));
    }

    #[test]
    fn test_number_pattern_json_export() {
        let env = eval_module("x = 42", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("test_number.json");

        export_number_pattern_to_json(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.starts_with('['));
        assert!(content.ends_with("]\n"));
        assert!(content.contains("\"value\": 42.000000"));
        assert!(content.contains("\"start_num\": 0"));
        assert!(content.contains("\"end_num\": 1"));
    }

    #[test]
    fn test_json_export_fails_with_zero_cycles() {
        let env = eval_module("x = bd", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
        let path = std::env::temp_dir().join("fail.json");

        let err = export_sample_pattern_to_json(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }
}
