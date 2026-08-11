//! The `chuck_export` module provides an exporter to the `ChucK` audio programming language.
//!
//! This exporter translates evaluated number patterns (representing MIDI pitches)
//! into a `.ck` `ChucK` script that sequences an oscillator.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a number pattern's evaluated events to a `ChucK` script.
///
/// Number patterns are assumed to represent MIDI pitch. The exporter generates
/// a simple sequence that drives a sine wave oscillator (`SinOsc`) in `ChucK`.
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
/// let path = std::env::temp_dir().join("export_music.ck");
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

    writeln!(file, "// Orpheus ChucK Music Export")?;
    writeln!(file, "// Cycles: {cycle_count}")?;
    writeln!(file)?;

    writeln!(file, "SinOsc osc => dac;")?;
    writeln!(file, "0.0 => osc.gain;")?;
    writeln!(file)?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_time = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let duration_s = end_time - start_time;

        if start_time > current_time {
            let sleep_duration = start_time - current_time;
            writeln!(file, "0.0 => osc.gain;")?;
            writeln!(file, "{sleep_duration}::second => now;")?;
        }

        let pitch = event.value;
        let frequency_hz = 440.0 * ((pitch - 69.0) / 12.0).exp2();

        writeln!(file, "0.5 => osc.gain;")?;
        writeln!(file, "{frequency_hz:.3} => osc.freq; // Note: {pitch}")?;
        writeln!(file, "{duration_s}::second => now;")?;

        current_time = end_time;
    }

    writeln!(file, "0.0 => osc.gain;")?;

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
    fn chuck_exporter_generates_script_for_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_chuck_output_{}.ck", unique_temp_suffix()));
        export_number_pattern_to_chuck(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("// Orpheus ChucK Music Export"));
        assert!(content.contains("SinOsc osc => dac;"));

        // C4 (60) is ~261.626 Hz
        assert!(content.contains("261.626 => osc.freq;"));
        // D4 (62) is ~293.665 Hz
        assert!(content.contains("293.665 => osc.freq;"));
        // E4 (64) is ~329.628 Hz
        assert!(content.contains("329.628 => osc.freq;"));

        assert!(content.contains("::second => now;"));
    }

    #[test]
    fn chuck_exporter_handles_rests() {
        let source = "pattern = 60 ~ 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_chuck_output_{}.ck", unique_temp_suffix()));
        export_number_pattern_to_chuck(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("// Orpheus ChucK Music Export"));
        assert!(content.contains("0.0 => osc.gain;"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
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
