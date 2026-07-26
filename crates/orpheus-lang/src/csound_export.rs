//! Exporter for `CSound` Score (`.sco`) format.
//!
//! This module exports patterns to `CSound` numeric score format.
//! Orpheus samples are mapped to integer instrument IDs, and parameters
//! are translated into standard `i` statement `p` fields (p1=instr, p2=start,
//! p3=dur, p4=amp, p5=pitch/rate, p6=pan).

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Maps a sample string to a `CSound` instrument integer ID.
#[must_use]
pub fn map_sample_to_instr(sample: &str) -> i32 {
    match sample {
        "bd" | "kick" => 1,
        "sn" | "snare" => 2,
        "hh" | "hat" => 3,
        "oh" | "openhat" => 4,
        "cp" | "clap" => 5,
        "bass" => 10,
        _ => 99,
    }
}

/// Exports a sample pattern to a `CSound` score file.
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
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "; Orpheus CSound Export")?;
    writeln!(file, "; Cycles: {cycle_count}")?;
    writeln!(file, "; Format: i p1(instr) p2(start) p3(dur) p4(amp) p5(rate) p6(pan)")?;
    writeln!(file)?;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let duration = f64::from(event.part.end()).mul_add(SECONDS_PER_CYCLE, -start_time);
        let instr = map_sample_to_instr(event.value.sample());

        let amp = event.value.gain();
        let rate = event.value.rate();
        let pan = event.value.pan();

        writeln!(
            file,
            "i {instr:>3} {start_time:>8.3} {duration:>8.3} {amp:>8.3} {rate:>8.3} {pan:>8.3}"
        )?;
    }

    Ok(())
}

/// Exports a number pattern to a `CSound` score file.
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
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "; Orpheus CSound Export (Number Pattern)")?;
    writeln!(file, "; Cycles: {cycle_count}")?;
    writeln!(file, "; Format: i p1(instr) p2(start) p3(dur) p4(pitch)")?;
    writeln!(file)?;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let duration = f64::from(event.part.end()).mul_add(SECONDS_PER_CYCLE, -start_time);
        // Default number pattern instrument is 100
        let instr = 100;
        let pitch = event.value;

        writeln!(
            file,
            "i {instr:>3} {start_time:>8.3} {duration:>8.3} {pitch:>8.3}"
        )?;
    }

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
    fn csound_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join(format!("test_csound_sample_output_{}.sco", unique_temp_suffix()));
        export_sample_pattern_to_csound(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("; Orpheus CSound Export"));
        assert!(content.contains("i   1"));
        assert!(content.contains("i   2"));
    }

    #[test]
    fn csound_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join(format!("test_csound_number_output_{}.sco", unique_temp_suffix()));
        export_number_pattern_to_csound(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("; Orpheus CSound Export (Number Pattern)"));
        assert!(content.contains("i 100"));
        assert!(content.contains("60.000"));
        assert!(content.contains("62.000"));
        assert!(content.contains("64.000"));
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
