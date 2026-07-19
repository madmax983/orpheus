//! The `python_export` module provides a Python script export format for evaluated patterns.
//!
//! This exporter generates a Python script containing lists of event dictionaries, which can be
//! useful for integrating Orpheus patterns into data science workflows, generative AI pipelines,
//! or other Python-based environments.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to a Python script file.
///
/// Assumes a default tempo of 120 BPM (1 cycle = 2 seconds) for time mapping.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_sample_pattern_to_python;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.py");
/// export_sample_pattern_to_python(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_python(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    // Python script requires events to be sorted by start time
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    // Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
    let seconds_per_cycle = 2.0;

    writeln!(file, "SEQUENCE = [")?;

    for event in &events {
        let start_sec = f64::from(event.part.start()) * seconds_per_cycle;
        let end_sec = f64::from(event.part.end()) * seconds_per_cycle;
        let sample = event.value.sample();

        writeln!(
            file,
            "    {{ \"start_time\": {start_sec:.3}, \"end_time\": {end_sec:.3}, \"sample\": \"{sample}\" }},"
        )?;
    }

    writeln!(file, "]")?;

    Ok(())
}

/// Exports a number pattern's evaluated events to a Python script file.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_python;
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("automation.py");
/// export_number_pattern_to_python(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_python(
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

    writeln!(file, "SEQUENCE = [")?;

    for event in &events {
        let start_sec = f64::from(event.part.start()) * seconds_per_cycle;
        let end_sec = f64::from(event.part.end()) * seconds_per_cycle;

        let value = event.value;
        writeln!(
            file,
            "    {{ \"start_time\": {start_sec:.3}, \"end_time\": {end_sec:.3}, \"value\": {value:.3} }},"
        )?;
    }

    writeln!(file, "]")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn python_exporter_generates_valid_format_for_samples() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_output.py");
        export_sample_pattern_to_python(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("SEQUENCE = ["));
        assert!(
            content.contains("{ \"start_time\": 0.000, \"end_time\": 1.000, \"sample\": \"bd\" },")
        );
        assert!(
            content.contains("{ \"start_time\": 1.000, \"end_time\": 2.000, \"sample\": \"sn\" },")
        );
        assert!(content.contains(']'));
    }

    #[test]
    fn python_exporter_generates_valid_format_for_numbers() {
        let source = "x = 1 2";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_number_output.py");
        export_number_pattern_to_python(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("SEQUENCE = ["));
        assert!(
            content.contains("{ \"start_time\": 0.000, \"end_time\": 1.000, \"value\": 1.000 },")
        );
        assert!(
            content.contains("{ \"start_time\": 1.000, \"end_time\": 2.000, \"value\": 2.000 },")
        );
        assert!(content.contains(']'));
    }
}
