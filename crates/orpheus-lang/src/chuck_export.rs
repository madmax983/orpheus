//! The `chuck_export` module provides an exporter to `ChucK` (`.ck`) scripts.
//!
//! This exporter generates a `.ck` script containing instructions
//! for evaluated Orpheus patterns, mapping Orpheus sample identifiers to
//! `SndBuf` and playing number patterns as pitch information via `SinOsc`.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a sample pattern's evaluated events to a `ChucK` script.
///
/// Each event translates into a wait and a `SndBuf` trigger.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_sample_pattern_to_chuck;
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
#[allow(clippy::cast_precision_loss)]
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
    // ChucK script requires sequential playback wait times
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "// Orpheus Sample Pattern Export")?;
    writeln!(file, "SndBuf buf => dac;")?;

    let mut current_time = 0.0;

    for event in events {
        let start_sec = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let delta = start_sec - current_time;

        if delta > 0.0 {
            writeln!(file, "{delta}::second => now;")?;
            current_time = start_sec;
        }

        writeln!(file, "\"{}.wav\" => buf.read;", event.value.sample())?;
        writeln!(file, "0 => buf.pos;")?;
    }

    let total_time = (cycle_count as f64) * SECONDS_PER_CYCLE;
    if total_time > current_time {
        writeln!(file, "{}::second => now;", total_time - current_time)?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a `ChucK` script.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_chuck;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_num.ck");
/// export_number_pattern_to_chuck(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(clippy::cast_precision_loss)]
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

    writeln!(file, "// Orpheus Number Pattern Export")?;
    writeln!(file, "SinOsc osc => dac;")?;
    writeln!(file, "0.0 => osc.gain;")?;

    let mut current_time = 0.0;

    for event in events {
        let start_sec = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_sec = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let delta = start_sec - current_time;

        if delta > 0.0 {
            writeln!(file, "{delta}::second => now;")?;
        }

        let midi_note = event.value;
        let freq = 440.0 * ((midi_note - 69.0) / 12.0).exp2();

        writeln!(file, "{freq:.2} => osc.freq;")?;
        writeln!(file, "0.5 => osc.gain;")?;

        let duration = end_sec - start_sec;
        writeln!(file, "{duration}::second => now;")?;
        writeln!(file, "0.0 => osc.gain;")?;

        current_time = end_sec;
    }

    let total_time = (cycle_count as f64) * SECONDS_PER_CYCLE;
    if total_time > current_time {
        writeln!(file, "{}::second => now;", total_time - current_time)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn chuck_exporter_generates_valid_format_for_samples() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_output.ck");
        export_sample_pattern_to_chuck(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("SndBuf buf => dac;"));
        assert!(content.contains("\"bd.wav\" => buf.read;"));
        assert!(content.contains("\"sn.wav\" => buf.read;"));
        assert!(content.contains("1::second => now;"));
    }

    #[test]
    fn chuck_exporter_generates_valid_format_for_numbers() {
        let source = "x = 69 71"; // A4 and B4
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_number_output.ck");
        export_number_pattern_to_chuck(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("SinOsc osc => dac;"));
        assert!(content.contains("440.00 => osc.freq;")); // A4 is 440 Hz
        assert!(content.contains("1::second => now;"));
        assert!(content.contains("0.0 => osc.gain;"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_chuck(pat, "test.ck", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_chuck(pat, "test.ck", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
