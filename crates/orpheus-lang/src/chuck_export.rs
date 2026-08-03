//! The `chuck_export` module provides an exporter to `ChucK` (`.ck`) scripts.
//!
//! This exporter generates a `.ck` script containing evaluated Orpheus patterns,
//! mapping Orpheus sample identifiers to standard sound files, and playing
//! number patterns as pitches.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a sample pattern's evaluated events to a `ChucK` script.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::chuck_export::export_sample_pattern_to_chuck;
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
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "// Orpheus `ChucK` Export")?;
    writeln!(file, "// Cycles: {cycle_count}")?;
    writeln!(file, "SndBuf buf => dac;")?;
    writeln!(file)?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;

        if start_time > current_time {
            let dur = start_time - current_time;
            writeln!(file, "{dur:.3}::second => now;")?;
            current_time = start_time;
        }

        let sample_name = event.value.sample();
        let gain = event.value.gain();
        let rate = event.value.rate();
        writeln!(file, "\"{sample_name}.wav\" => buf.read;")?;
        writeln!(file, "{gain:.3} => buf.gain;")?;
        writeln!(file, "{rate:.3} => buf.rate;")?;
        writeln!(file, "0 => buf.pos;")?;
    }

    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * SECONDS_PER_CYCLE;
    if total_time > current_time {
        writeln!(file, "{:.3}::second => now;", total_time - current_time)?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a `ChucK` script.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::chuck_export::export_number_pattern_to_chuck;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_num.ck");
/// export_number_pattern_to_chuck(pattern, &path, 2).unwrap();
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
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "// Orpheus `ChucK` Export")?;
    writeln!(file, "// Cycles: {cycle_count}")?;
    writeln!(file, "SinOsc osc => dac;")?;
    writeln!(file)?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_time = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let duration = end_time - start_time;

        if start_time > current_time {
            let dur = start_time - current_time;
            writeln!(file, "0.0 => osc.gain;")?;
            writeln!(file, "{dur:.3}::second => now;")?;
            current_time = start_time;
        }

        let pitch = event.value;
        writeln!(file, "Std.mtof({pitch:.3}) => osc.freq;")?;
        writeln!(file, "0.5 => osc.gain;")?;
        writeln!(file, "{duration:.3}::second => now;")?;
        current_time += duration;
    }

    writeln!(file, "0.0 => osc.gain;")?;

    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * SECONDS_PER_CYCLE;
    if total_time > current_time {
        writeln!(file, "{:.3}::second => now;", total_time - current_time)?;
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
    fn chuck_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_sample_output_{}.ck", unique_temp_suffix()));
        export_sample_pattern_to_chuck(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("// Orpheus `ChucK` Export"));
        assert!(content.contains("\"bd.wav\" => buf.read;"));
        assert!(content.contains("\"sn.wav\" => buf.read;"));
        assert!(content.contains("1.000::second => now;"));
    }

    #[test]
    fn chuck_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_number_output_{}.ck", unique_temp_suffix()));
        export_number_pattern_to_chuck(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("// Orpheus `ChucK` Export"));
        assert!(content.contains("Std.mtof(60.000) => osc.freq;"));
        assert!(content.contains("0.667::second => now;"));
    }

    #[test]
    fn export_sample_pattern_zero_cycles() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();
        let path =
            std::env::temp_dir().join(format!("test_zero_sample_{}.ck", unique_temp_suffix()));

        let err = export_sample_pattern_to_chuck(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }

    #[test]
    fn export_number_pattern_zero_cycles() {
        let source = "pattern = 1 2";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();
        let path =
            std::env::temp_dir().join(format!("test_zero_number_{}.ck", unique_temp_suffix()));

        let err = export_number_pattern_to_chuck(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }
}
