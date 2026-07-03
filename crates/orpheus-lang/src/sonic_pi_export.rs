//! The `sonic_pi_export` module provides an exporter to Sonic Pi (`.rb`) scripts.
//!
//! This exporter generates a Ruby script for Sonic Pi containing threads
//! for evaluated Orpheus patterns, triggering a default synth for notes
//! and conceptual samples for sample events.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
// Sonic Pi's default BPM is 60, where 1 sleep unit = 1 second.
// For 120 BPM, 1 cycle = 2.0 seconds, so sleep 1.0 = 1.0 second.
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a sample pattern's evaluated events to a Sonic Pi script.
///
/// # Errors
/// Returns an `EvalError` if the pattern fails to render or query properly,
/// or if writing the output file fails.
pub fn export_sample_pattern_to_sonic_pi(
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

    writeln!(file, "# Orpheus Sonic Pi Export")?;
    writeln!(file, "# =========================")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(file)?;

    writeln!(file, "in_thread do")?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;

        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(file, "  sleep {sleep_dur:.3}")?;
            current_time = start_time;
        }

        let sample = event.value.sample();
        let gain = event.value.gain();
        let pan = event.value.pan();
        let rate = event.value.rate();

        writeln!(
            file,
            "  sample :{sample}, amp: {gain:.3}, pan: {pan:.3}, rate: {rate:.3}"
        )?;
    }

    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * SECONDS_PER_CYCLE;
    if total_time > current_time {
        let remaining = total_time - current_time;
        writeln!(file, "  sleep {remaining:.3}")?;
    }

    writeln!(file, "end")?;

    Ok(())
}

/// Exports a number pattern's evaluated events to a Sonic Pi script.
///
/// # Errors
/// Returns an `EvalError` if the pattern fails to render or query properly,
/// or if writing the output file fails.
pub fn export_number_pattern_to_sonic_pi(
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

    writeln!(file, "# Orpheus Sonic Pi Export")?;
    writeln!(file, "# =========================")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(file)?;

    writeln!(file, "in_thread do")?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_time = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let duration = end_time - start_time;

        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(file, "  sleep {sleep_dur:.3}")?;
            current_time = start_time;
        }

        let pitch = event.value;
        writeln!(file, "  play {pitch:.3}, amp: 0.5, sustain: {duration:.3}")?;
    }

    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * SECONDS_PER_CYCLE;
    if total_time > current_time {
        let remaining = total_time - current_time;
        writeln!(file, "  sleep {remaining:.3}")?;
    }

    writeln!(file, "end")?;

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
    fn sonic_pi_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_sample_output_{}.rb", unique_temp_suffix()));
        export_sample_pattern_to_sonic_pi(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Orpheus Sonic Pi Export"));
        assert!(content.contains("sleep 1.000"));
        assert!(content.contains("sample :bd"));
        assert!(content.contains("sample :sn"));
    }

    #[test]
    fn sonic_pi_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_number_output_{}.rb", unique_temp_suffix()));
        export_number_pattern_to_sonic_pi(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Orpheus Sonic Pi Export"));
        assert!(content.contains("sleep 0.667"));
        assert!(content.contains("play 60.000"));
        assert!(content.contains("play 62.000"));
        assert!(content.contains("play 64.000"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_sonic_pi(pat, "test.rb", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_sonic_pi(pat, "test.rb", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
