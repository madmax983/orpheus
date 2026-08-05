//! The `csound_export` module provides an exporter to `CSound` (`.csd`) format.
//!
//! This exporter generates a complete `CSound` unified file containing musical notes
//! mapped from a number pattern, allowing rendering to standard `CSound` engines.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to a `CSound` file.
///
/// Sample names are rendered as comments alongside the events in the score.
///
/// # Errors
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
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    let cycle_duration = 2.0;

    writeln!(file, "<CsoundSynthesizer>")?;
    writeln!(file, "<CsOptions>")?;
    writeln!(file, "-odac")?;
    writeln!(file, "</CsOptions>")?;
    writeln!(file, "<CsInstruments>")?;
    writeln!(file, "sr = 44100")?;
    writeln!(file, "ksmps = 32")?;
    writeln!(file, "nchnls = 2")?;
    writeln!(file, "0dbfs = 1.0")?;
    writeln!(file)?;
    writeln!(file, "instr 1")?;
    writeln!(file, "  ; Placeholder synth for samples")?;
    writeln!(file, "  iamp = p4")?;
    writeln!(file, "  icps = 440")?;
    writeln!(file, "  aenv = linen(iamp, 0.05, p3, 0.1)")?;
    writeln!(file, "  aout = vco2(aenv, icps)")?;
    writeln!(file, "  outs aout, aout")?;
    writeln!(file, "endin")?;
    writeln!(file, "</CsInstruments>")?;
    writeln!(file, "<CsScore>")?;

    for event in events {
        let start_time = f64::from(event.part.start()) * cycle_duration;
        let end_time = f64::from(event.part.end()) * cycle_duration;
        let duration = end_time - start_time;
        let gain = event.value.gain();
        let sample = event.value.sample();

        writeln!(
            file,
            "i 1 {start_time:.4} {duration:.4} {gain:.4} ; {sample}"
        )?;
    }

    writeln!(file, "</CsScore>")?;
    writeln!(file, "</CsoundSynthesizer>")?;

    Ok(())
}

/// Exports a number pattern's evaluated events to a `CSound` file.
///
/// Number patterns are assumed to represent MIDI note numbers.
///
/// # Errors
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
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    let cycle_duration = 2.0;

    writeln!(file, "<CsoundSynthesizer>")?;
    writeln!(file, "<CsOptions>")?;
    writeln!(file, "-odac")?;
    writeln!(file, "</CsOptions>")?;
    writeln!(file, "<CsInstruments>")?;
    writeln!(file, "sr = 44100")?;
    writeln!(file, "ksmps = 32")?;
    writeln!(file, "nchnls = 2")?;
    writeln!(file, "0dbfs = 1.0")?;
    writeln!(file)?;
    writeln!(file, "instr 1")?;
    writeln!(file, "  icps = cpsmidinn(p4)")?;
    writeln!(file, "  iamp = 0.5")?;
    writeln!(file, "  aenv = linen(iamp, 0.05, p3, 0.1)")?;
    writeln!(file, "  aout = vco2(aenv, icps)")?;
    writeln!(file, "  outs aout, aout")?;
    writeln!(file, "endin")?;
    writeln!(file, "</CsInstruments>")?;
    writeln!(file, "<CsScore>")?;

    for event in events {
        let start_time = f64::from(event.part.start()) * cycle_duration;
        let end_time = f64::from(event.part.end()) * cycle_duration;
        let duration = end_time - start_time;
        let pitch = event.value;

        writeln!(file, "i 1 {start_time:.4} {duration:.4} {pitch:.4}")?;
    }

    writeln!(file, "</CsScore>")?;
    writeln!(file, "</CsoundSynthesizer>")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};
    use std::fs;
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
    fn csound_exporter_generates_valid_format_for_numbers() {
        let source = "x = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_csound_output_{}.csd", unique_temp_suffix()));
        export_number_pattern_to_csound(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("<CsoundSynthesizer>"));
        assert!(content.contains("instr 1"));
        assert!(content.contains("i 1 0.0000 0.6667 60.0000"));
        assert!(content.contains("i 1 0.6667 0.6667 62.0000"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn csound_exporter_generates_valid_format_for_samples() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join(format!(
            "test_csound_sample_output_{}.csd",
            unique_temp_suffix()
        ));
        export_sample_pattern_to_csound(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("<CsoundSynthesizer>"));
        assert!(content.contains("instr 1"));
        assert!(content.contains("i 1 0.0000 1.0000 1.0000 ; bd"));
        assert!(content.contains("i 1 1.0000 1.0000 1.0000 ; sn"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();
        assert_eq!(
            super::export_number_pattern_to_csound(pat, "test.csd", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();
        assert_eq!(
            super::export_sample_pattern_to_csound(pat, "test.csd", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
