//! The `txt` module provides a plain text export format for evaluated patterns.
//!
//! This exporter generates a human-readable chronological summary of events,
//! similar to a tracker sequence or playlist.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to a plain text file.
///
/// Each line in the generated file represents an event with its timing
/// and synthesized parameters.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_sample_pattern_to_txt};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.txt");
/// export_sample_pattern_to_txt(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_txt(
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

    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);

    writeln!(file, "Orpheus Sample Pattern Export")?;
    writeln!(file, "=============================")?;
    writeln!(file, "Cycles: {cycle_count}")?;
    writeln!(file)?;

    for event in events {
        let start = f64::from(event.part.start());
        let end = f64::from(event.part.end());

        write!(
            file,
            "[{start:.3} -> {end:.3}] {} (gain: {:.2}, pan: {:.2}, rate: {:.2}",
            event.value.sample(),
            event.value.gain(),
            event.value.pan(),
            event.value.rate()
        )?;

        if let Some(hpf) = event.value.hpf_cutoff_hz() {
            write!(file, ", hpf: {hpf:.2}")?;
        }
        if let Some(lpf) = event.value.lpf_cutoff_hz() {
            write!(file, ", lpf: {lpf:.2}")?;
        }

        writeln!(file, ")")?;
    }

    file.flush()?;

    Ok(())
}

/// Exports a number pattern's evaluated events to a plain text file.
///
/// Each line in the generated file represents an event with its timing
/// and numeric value.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_txt};
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_num.txt");
/// export_number_pattern_to_txt(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_txt(
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

    let mut file = std::io::BufWriter::new(std::fs::File::create(path)?);

    writeln!(file, "Orpheus Number Pattern Export")?;
    writeln!(file, "=============================")?;
    writeln!(file, "Cycles: {cycle_count}")?;
    writeln!(file)?;

    for event in events {
        let start = f64::from(event.part.start());
        let end = f64::from(event.part.end());

        writeln!(file, "[{start:.3} -> {end:.3}] value: {:.3}", event.value)?;
    }

    file.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn txt_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_sample_output.txt");
        export_sample_pattern_to_txt(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Orpheus Sample Pattern Export"));
        assert!(content.contains("Cycles: 1"));
        assert!(content.contains("[0.000 -> 0.500] bd (gain: 1.00, pan: 0.00, rate: 1.00)"));
        assert!(content.contains("[0.500 -> 1.000] sn (gain: 1.00, pan: 0.00, rate: 1.00)"));
    }

    #[test]
    fn txt_exporter_generates_number_pattern() {
        let source = "pattern = 1 2 3";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_number_output.txt");
        export_number_pattern_to_txt(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Orpheus Number Pattern Export"));
        assert!(content.contains("Cycles: 1"));
        assert!(content.contains("[0.000 -> 0.333] value: 1.000"));
        assert!(content.contains("[0.333 -> 0.667] value: 2.000"));
        assert!(content.contains("[0.667 -> 1.000] value: 3.000"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_txt(pat, "test.txt", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_txt(pat, "test.txt", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
