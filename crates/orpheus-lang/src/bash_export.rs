//! The `bash_export` module provides an exporter to bash scripts.
//!
//! This exporter generates a `#!/bin/bash` script containing a sequence
//! of `sleep` and `play` (from `SoX`) commands to render the evaluated patterns.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
const SECS_PER_CYCLE: f64 = 2.0;

/// Exports a sample pattern's evaluated events to a bash script.
///
/// Each line in the generated script represents a sleep or play command
/// with its timing mapped to the bash environment.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::bash_export::export_sample_pattern_to_bash;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.sh");
/// export_sample_pattern_to_bash(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_bash(
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

    writeln!(file, "#!/bin/bash")?;
    writeln!(file, "# Orpheus Bash Export")?;
    writeln!(file, "# =========================")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(
        file,
        "# Note: Requires SoX (`play` command) to be installed."
    )?;
    writeln!(file)?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECS_PER_CYCLE;

        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(file, "sleep {sleep_dur:.3}")?;
            current_time = start_time;
        }

        let sample = event.value.sample();
        let gain = event.value.gain();

        // Run play in the background for polyphony
        writeln!(file, "play -q {sample}.wav vol {gain:.3} &")?;
    }

    // Sleep remaining time of the sequence
    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * SECS_PER_CYCLE;
    if total_time > current_time {
        writeln!(file, "sleep {:.3}", total_time - current_time)?;
    }

    // Wait for all background play commands to finish
    writeln!(file, "wait")?;

    Ok(())
}

/// Converts a MIDI note number to its corresponding frequency in Hertz.
#[must_use]
pub fn midi_to_hz(midi: f64) -> f64 {
    440.0 * ((midi - 69.0) / 12.0).exp2()
}

/// Exports a number pattern's evaluated events to a bash script.
///
/// Number patterns are assumed to represent pitch, and map to `play -n synth` statements
/// in bash via `SoX`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::bash_export::export_number_pattern_to_bash;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_num.sh");
/// export_number_pattern_to_bash(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_bash(
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

    writeln!(file, "#!/bin/bash")?;
    writeln!(file, "# Orpheus Bash Export")?;
    writeln!(file, "# =========================")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(
        file,
        "# Note: Requires SoX (`play` command) to be installed."
    )?;
    writeln!(file)?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECS_PER_CYCLE;
        let end_time = f64::from(event.part.end()) * SECS_PER_CYCLE;
        let duration = end_time - start_time;

        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(file, "sleep {sleep_dur:.3}")?;
            current_time = start_time;
        }

        let pitch = event.value;
        let hz = midi_to_hz(pitch);

        // Run play in the background for polyphony
        writeln!(file, "play -qn synth {duration:.3} sine {hz:.3} &")?;
    }

    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * SECS_PER_CYCLE;
    if total_time > current_time {
        writeln!(file, "sleep {:.3}", total_time - current_time)?;
    }

    // Wait for all background play commands to finish
    writeln!(file, "wait")?;

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
    fn bash_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_sample_output_{}.sh", unique_temp_suffix()));
        export_sample_pattern_to_bash(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("#!/bin/bash"));
        assert!(content.contains("sleep 1.000"));
        assert!(content.contains("play -q bd.wav"));
        assert!(content.contains("play -q sn.wav"));
        assert!(content.contains("wait"));
    }

    #[test]
    fn bash_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_number_output_{}.sh", unique_temp_suffix()));
        export_number_pattern_to_bash(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("#!/bin/bash"));
        assert!(content.contains("sleep 0.667"));
        assert!(content.contains("play -qn synth 0.667 sine 261.626"));
        assert!(content.contains("wait"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_bash(pat, "test.sh", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_bash(pat, "test.sh", 0)
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
        let path = std::env::temp_dir().join("test_zero_sample.sh");

        let err = export_sample_pattern_to_bash(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }

    #[test]
    fn export_number_pattern_zero_cycles() {
        let source = "pattern = fast(2, 1 2)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_number.sh");

        let err = export_number_pattern_to_bash(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }
}
