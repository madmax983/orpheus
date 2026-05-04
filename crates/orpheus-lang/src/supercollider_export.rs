//! The `supercollider_export` module provides an exporter to `SuperCollider` (`.scd`) scripts.
//!
//! This exporter generates a `.scd` script containing `Task` blocks
//! for evaluated Orpheus patterns, triggering a default Synth for notes
//! and a conceptual sample synth for samples.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a sample pattern's evaluated events to a `SuperCollider` script.
///
/// Each event translates into a wait and a Synth call.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_sample_pattern_to_supercollider;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.scd");
/// export_sample_pattern_to_supercollider(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_supercollider(
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

    writeln!(file, "// Orpheus `SuperCollider` Export")?;
    writeln!(file, "// =========================")?;
    writeln!(file, "// Cycles: {cycle_count}")?;
    writeln!(file)?;

    writeln!(file, "~orpheus_samples = Task({{")?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;

        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(file, "    {sleep_dur:.3}.wait;")?;
            current_time = start_time;
        }

        let sample = event.value.sample();
        let gain = event.value.gain();
        let pan = event.value.pan();
        let rate = event.value.rate();

        writeln!(
            file,
            "    Synth(\\play_sample, [\\bufname, \\{sample}, \\amp, {gain:.3}, \\pan, {pan:.3}, \\rate, {rate:.3}]);"
        )?;
    }

    // Wait remaining time of the sequence to allow looping
    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * SECONDS_PER_CYCLE;
    if total_time > current_time {
        writeln!(file, "    {:.3}.wait;", total_time - current_time)?;
    }

    writeln!(file, "}}).play;")?;

    Ok(())
}

/// Exports a number pattern's evaluated events to a `SuperCollider` script.
///
/// Number patterns are assumed to represent pitch, and map to a default Synth
/// in `SuperCollider`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_supercollider;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_num.scd");
/// export_number_pattern_to_supercollider(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_supercollider(
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

    writeln!(file, "// Orpheus `SuperCollider` Export")?;
    writeln!(file, "// =========================")?;
    writeln!(file, "// Cycles: {cycle_count}")?;
    writeln!(file)?;

    writeln!(file, "~orpheus_notes = Task({{")?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_time = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let duration = end_time - start_time;

        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(file, "    {sleep_dur:.3}.wait;")?;
            current_time = start_time;
        }

        let pitch = event.value;
        writeln!(
            file,
            "    Synth(\\default, [\\freq, {pitch:.3}.midicps, \\amp, 0.5, \\sustain, {duration:.3}]);"
        )?;
    }

    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * SECONDS_PER_CYCLE;
    if total_time > current_time {
        writeln!(file, "    {:.3}.wait;", total_time - current_time)?;
    }

    writeln!(file, "}}).play;")?;

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
    fn supercollider_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_sample_output_{}.scd", unique_temp_suffix()));
        export_sample_pattern_to_supercollider(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("// Orpheus `SuperCollider` Export"));
        assert!(content.contains("1.000.wait;"));
        assert!(content.contains("Synth(\\play_sample"));
        assert!(content.contains("\\bd"));
        assert!(content.contains("\\sn"));
    }

    #[test]
    fn supercollider_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_number_output_{}.scd", unique_temp_suffix()));
        export_number_pattern_to_supercollider(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("// Orpheus `SuperCollider` Export"));
        assert!(content.contains("0.667.wait;"));
        assert!(content.contains("Synth(\\default"));
        assert!(content.contains("60.000.midicps"));
        assert!(content.contains("62.000.midicps"));
        assert!(content.contains("64.000.midicps"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_supercollider(pat, "test.scd", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_supercollider(pat, "test.scd", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
