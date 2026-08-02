//! The `chuck_export` module provides an exporter to `ChucK` (`.ck`) scripts.
//!
//! This exporter generates a `ChucK` script containing arrays of timing and values,
//! allowing Orpheus patterns to be used in the `ChucK` audio programming language.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to a `ChucK` script file.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_sample_pattern_to_chuck};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.ck");
/// export_sample_pattern_to_chuck(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_chuck(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    // Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
    let seconds_per_cycle = 2.0;

    writeln!(file, "// Orpheus Sample Pattern Export")?;
    writeln!(file, "[")?;

    let mut starts = Vec::new();
    let mut ends = Vec::new();
    let mut samples = Vec::new();

    for event in &events {
        let start_sec = f64::from(event.part.start()) * seconds_per_cycle;
        let end_sec = f64::from(event.part.end()) * seconds_per_cycle;
        starts.push(start_sec);
        ends.push(end_sec);
        samples.push(event.value.sample());
    }

    // Write start times
    for (i, start) in starts.iter().enumerate() {
        if i == starts.len() - 1 {
            writeln!(file, "    {start:.3}")?;
        } else {
            write!(file, "    {start:.3}, ")?;
        }
    }
    writeln!(file, "] @=> float start_times[];\n")?;

    // Write end times
    writeln!(file, "[")?;
    for (i, end) in ends.iter().enumerate() {
        if i == ends.len() - 1 {
            writeln!(file, "    {end:.3}")?;
        } else {
            write!(file, "    {end:.3}, ")?;
        }
    }
    writeln!(file, "] @=> float end_times[];\n")?;

    // Write samples
    writeln!(file, "[")?;
    for (i, sample) in samples.iter().enumerate() {
        if i == samples.len() - 1 {
            writeln!(file, "    \"{sample}\"")?;
        } else {
            write!(file, "    \"{sample}\", ")?;
        }
    }
    writeln!(file, "] @=> string samples[];\n")?;

    Ok(())
}

/// Exports a number pattern's evaluated events to a `ChucK` script file.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_chuck};
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("automation.ck");
/// export_number_pattern_to_chuck(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_chuck(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    let seconds_per_cycle = 2.0;

    writeln!(file, "// Orpheus Number Pattern Export")?;

    let mut starts = Vec::new();
    let mut ends = Vec::new();
    let mut values = Vec::new();

    for event in &events {
        let start_sec = f64::from(event.part.start()) * seconds_per_cycle;
        let end_sec = f64::from(event.part.end()) * seconds_per_cycle;
        starts.push(start_sec);
        ends.push(end_sec);
        values.push(event.value);
    }

    writeln!(file, "[")?;
    // Write start times
    for (i, start) in starts.iter().enumerate() {
        if i == starts.len() - 1 {
            writeln!(file, "    {start:.3}")?;
        } else {
            write!(file, "    {start:.3}, ")?;
        }
    }
    writeln!(file, "] @=> float start_times[];\n")?;

    // Write end times
    writeln!(file, "[")?;
    for (i, end) in ends.iter().enumerate() {
        if i == ends.len() - 1 {
            writeln!(file, "    {end:.3}")?;
        } else {
            write!(file, "    {end:.3}, ")?;
        }
    }
    writeln!(file, "] @=> float end_times[];\n")?;

    // Write values
    writeln!(file, "[")?;
    for (i, value) in values.iter().enumerate() {
        if i == values.len() - 1 {
            writeln!(file, "    {value:.3}")?;
        } else {
            write!(file, "    {value:.3}, ")?;
        }
    }
    writeln!(file, "] @=> float values[];\n")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_suffix() -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        format!("{timestamp}-{counter}")
    }

    #[test]
    fn chuck_exporter_generates_valid_format_for_samples() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join(format!("test_output_{}.ck", unique_temp_suffix()));
        export_sample_pattern_to_chuck(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("// Orpheus Sample Pattern Export"));
        assert!(content.contains("0.000,     1.000"));
        assert!(content.contains("1.000,     2.000"));
        assert!(content.contains("\"bd\",     \"sn\""));
        assert!(content.contains("] @=> float start_times[];"));
        assert!(content.contains("] @=> string samples[];"));
    }

    #[test]
    fn chuck_exporter_generates_valid_format_for_numbers() {
        let source = "x = 1 2";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_number_output_{}.ck", unique_temp_suffix()));
        export_number_pattern_to_chuck(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("// Orpheus Number Pattern Export"));
        assert!(content.contains("0.000,     1.000"));
        assert!(content.contains("1.000,     2.000"));
        assert!(content.contains("] @=> float values[];"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();
        let path = std::env::temp_dir().join(format!("test_error_{}.ck", unique_temp_suffix()));

        assert_eq!(
            super::export_sample_pattern_to_chuck(pat, &path, 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();
        let path2 = std::env::temp_dir().join(format!("test_error2_{}.ck", unique_temp_suffix()));

        assert_eq!(
            super::export_number_pattern_to_chuck(pat, &path2, 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
