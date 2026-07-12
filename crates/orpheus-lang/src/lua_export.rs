//! The `lua_export` module provides a Lua script export format for evaluated patterns.
//!
//! This exporter generates a Lua script containing tables of events, which can be
//! useful for integrating Orpheus patterns into game engines (like LÖVE) or other
//! scripting environments.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to a Lua script file.
///
/// Assumes a default tempo of 120 BPM (1 cycle = 2 seconds) for time mapping.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_sample_pattern_to_lua};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.lua");
/// export_sample_pattern_to_lua(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_lua(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    // Lua script requires events to be sorted by start time
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    // Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
    let seconds_per_cycle = 2.0;

    writeln!(file, "local sequence = {{")?;

    for event in &events {
        let start_sec = f64::from(event.part.start()) * seconds_per_cycle;
        let end_sec = f64::from(event.part.end()) * seconds_per_cycle;
        let sample = event.value.sample();

        writeln!(
            file,
            "    {{ start_time = {start_sec:.3}, end_time = {end_sec:.3}, sample = \"{sample}\" }},"
        )?;
    }

    writeln!(file, "}}")?;
    writeln!(file, "return sequence")?;

    Ok(())
}

/// Exports a number pattern's evaluated events to a Lua script file.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_lua};
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("automation.lua");
/// export_number_pattern_to_lua(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_lua(
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

    writeln!(file, "local sequence = {{")?;

    for event in &events {
        let start_sec = f64::from(event.part.start()) * seconds_per_cycle;
        let end_sec = f64::from(event.part.end()) * seconds_per_cycle;

        let value = event.value;
        writeln!(
            file,
            "    {{ start_time = {start_sec:.3}, end_time = {end_sec:.3}, value = {value:.3} }},"
        )?;
    }

    writeln!(file, "}}")?;
    writeln!(file, "return sequence")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn lua_exporter_generates_valid_format_for_samples() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_output.lua");
        export_sample_pattern_to_lua(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("local sequence = {"));
        assert!(content.contains("{ start_time = 0.000, end_time = 1.000, sample = \"bd\" },"));
        assert!(content.contains("{ start_time = 1.000, end_time = 2.000, sample = \"sn\" },"));
        assert!(content.contains("return sequence"));
    }

    #[test]
    fn lua_exporter_generates_valid_format_for_numbers() {
        let source = "x = 1 2";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_number_output.lua");
        export_number_pattern_to_lua(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("local sequence = {"));
        assert!(content.contains("{ start_time = 0.000, end_time = 1.000, value = 1.000 },"));
        assert!(content.contains("{ start_time = 1.000, end_time = 2.000, value = 2.000 },"));
        assert!(content.contains("return sequence"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_lua(pat, "test.lua", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_lua(pat, "test.lua", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}

#[cfg(test)]
mod test_zero_cycle {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn export_sample_pattern_zero_cycles() {
        let source = "pattern = fast(2, bd sn)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_sample.lua");

        let err = export_sample_pattern_to_lua(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }

    #[test]
    fn export_number_pattern_zero_cycles() {
        let source = "pattern = fast(2, 1 2)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_number.lua");

        let err = export_number_pattern_to_lua(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }
}
