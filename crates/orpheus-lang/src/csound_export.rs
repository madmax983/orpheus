//! The `csound_export` module provides a `CSound` Score (`.sco`) export format.
//!
//! This exporter generates a standard `CSound` score file containing `i` (instrument)
//! statements, which can be used to render Orpheus patterns using the classic
//! `CSound` synthesis engine.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a sample pattern's evaluated events to a `CSound` Score file.
///
/// Uses instrument 1 (`i 1`) and outputs start time, duration, gain, pan, and rate.
/// The sample name is included as a comment.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_sample_pattern_to_csound;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.sco");
/// export_sample_pattern_to_csound(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_csound(
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

    writeln!(file, "; Orpheus CSound Score Export")?;
    writeln!(file, "; ===========================")?;
    writeln!(file, "; Cycles: {cycle_count}")?;
    writeln!(file)?;
    writeln!(file, "; i_num start dur gain pan rate ; sample")?;

    for event in events {
        let start_sec = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let dur_sec =
            (f64::from(event.part.end()) - f64::from(event.part.start())) * SECONDS_PER_CYCLE;
        let gain = event.value.gain();
        let pan = event.value.pan();
        let rate = event.value.rate();
        let sample = event.value.sample();

        writeln!(
            file,
            "i 1 {start_sec:.3} {dur_sec:.3} {gain:.3} {pan:.3} {rate:.3} ; {sample}"
        )?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a `CSound` Score file.
///
/// Uses instrument 1 (`i 1`) and outputs start time, duration, and the value (as pitch/parameter).
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_csound;
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("automation.sco");
/// export_number_pattern_to_csound(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_csound(
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

    writeln!(file, "; Orpheus CSound Score Export")?;
    writeln!(file, "; ===========================")?;
    writeln!(file, "; Cycles: {cycle_count}")?;
    writeln!(file)?;
    writeln!(file, "; i_num start dur value")?;

    for event in events {
        let start_sec = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let dur_sec =
            (f64::from(event.part.end()) - f64::from(event.part.start())) * SECONDS_PER_CYCLE;
        let value = event.value;

        writeln!(file, "i 1 {start_sec:.3} {dur_sec:.3} {value:.3}")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn csound_exporter_generates_valid_format_for_samples() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_csound.sco");
        export_sample_pattern_to_csound(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("; Orpheus CSound Score Export"));
        assert!(content.contains("i 1 0.000 1.000 1.000 0.000 1.000 ; bd"));
        assert!(content.contains("i 1 1.000 1.000 1.000 0.000 1.000 ; sn"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn csound_exporter_generates_valid_format_for_numbers() {
        let source = "x = 1 2";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_csound_number.sco");
        export_number_pattern_to_csound(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("; Orpheus CSound Score Export"));
        assert!(content.contains("i 1 0.000 1.000 1.000"));
        assert!(content.contains("i 1 1.000 1.000 2.000"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_csound(pat, "test.sco", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_csound(pat, "test.sco", 0)
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
        let path = std::env::temp_dir().join("test_zero_sample.sco");

        let err = export_sample_pattern_to_csound(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }

    #[test]
    fn export_number_pattern_zero_cycles() {
        let source = "pattern = fast(2, 1 2)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_number.sco");

        let err = export_number_pattern_to_csound(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }
}
