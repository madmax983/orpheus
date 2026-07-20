//! The `mermaid` module provides export functionality for Mermaid charts.
//!
//! This allows visualizing evaluated pattern timelines (such as drum sequences
//! or synth part activations) as Gantt charts natively rendered by Mermaid.js.
use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to a Mermaid Gantt chart.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_sample_pattern_to_mermaid_gantt;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("piano_roll.mermaid");
/// export_sample_pattern_to_mermaid_gantt(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn export_sample_pattern_to_mermaid_gantt(
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

    writeln!(file, "gantt")?;
    writeln!(file, "    title Orpheus Pattern")?;
    writeln!(file, "    dateFormat  x")?;
    writeln!(file, "    axisFormat %S.%L")?;

    let seconds_per_cycle = 2.0;

    let mut sections: std::collections::BTreeMap<&str, Vec<_>> = std::collections::BTreeMap::new();
    for event in &events {
        sections
            .entry(event.value.sample())
            .or_default()
            .push(event);
    }

    for (sample, sample_events) in sections {
        writeln!(file, "    section {sample}")?;
        for event in sample_events {
            let start_ms =
                (f64::from(event.part.start()) * seconds_per_cycle * 1000.0).round() as u64;
            let end_ms = (f64::from(event.part.end()) * seconds_per_cycle * 1000.0).round() as u64;

            // Mermaid requires unique IDs or just task name and length
            // Syntax: task_name : start_time, end_time
            writeln!(file, "    {sample} :{start_ms}, {end_ms}")?;
        }
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a Mermaid Gantt chart.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_mermaid_gantt;
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("automation.mermaid");
/// export_number_pattern_to_mermaid_gantt(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn export_number_pattern_to_mermaid_gantt(
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

    writeln!(file, "gantt")?;
    writeln!(file, "    title Orpheus Number Pattern")?;
    writeln!(file, "    dateFormat  x")?;
    writeln!(file, "    axisFormat %S.%L")?;

    let seconds_per_cycle = 2.0;

    writeln!(file, "    section Values")?;
    for event in &events {
        let start_ms = (f64::from(event.part.start()) * seconds_per_cycle * 1000.0).round() as u64;
        let end_ms = (f64::from(event.part.end()) * seconds_per_cycle * 1000.0).round() as u64;

        // Mermaid requires unique IDs or just task name and length
        // Syntax: task_name : start_time, end_time
        writeln!(file, "    {:.3} :{start_ms}, {end_ms}", event.value)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn mermaid_exporter_generates_valid_format() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_mermaid.mmd");
        export_sample_pattern_to_mermaid_gantt(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("gantt"));
        assert!(content.contains("section bd"));
        assert!(content.contains("bd :0, 1000"));
        assert!(content.contains("section sn"));
        assert!(content.contains("sn :1000, 2000"));
    }

    #[test]
    fn mermaid_exporter_generates_valid_format_for_number_pattern() {
        let source = "x = 1 2 3";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_mermaid_num.mmd");
        export_number_pattern_to_mermaid_gantt(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("gantt"));
        assert!(content.contains("section Values"));
        assert!(content.contains("1.000 :0, 667"));
        assert!(content.contains("2.000 :667, 1333"));
        assert!(content.contains("3.000 :1333, 2000"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_mermaid_gantt(pat, "test.mmd", 0)
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
        let path = std::env::temp_dir().join("test_zero_sample.mermaid");

        let err = export_sample_pattern_to_mermaid_gantt(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }

    #[test]
    fn export_number_pattern_zero_cycles() {
        let source = "pattern = fast(2, 1 2)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_number.mermaid");

        let err = export_number_pattern_to_mermaid_gantt(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }
}
