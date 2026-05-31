//! The `godot_export` module provides an exporter to Godot `GDScript` (`.gd`) scripts.
//!
//! This exporter generates a `.gd` script containing async functions
//! for evaluated Orpheus patterns, emitting signals for samples and notes.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a sample pattern's evaluated events to a Godot `GDScript` file.
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_godot(
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

    writeln!(file, "extends Node")?;
    writeln!(file)?;
    writeln!(file, "signal play_sample(sample_name, gain, pan, rate)")?;
    writeln!(file)?;
    writeln!(file, "func _ready():")?;
    writeln!(file, "\tplay_pattern()")?;
    writeln!(file)?;
    writeln!(file, "func play_pattern():")?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;

        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(
                file,
                "\tawait get_tree().create_timer({sleep_dur:.3}).timeout"
            )?;
            current_time = start_time;
        }

        let sample = event.value.sample();
        let gain = event.value.gain();
        let pan = event.value.pan();
        let rate = event.value.rate();

        writeln!(
            file,
            "\tplay_sample.emit(\"{sample}\", {gain:.3}, {pan:.3}, {rate:.3})"
        )?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a Godot `GDScript` file.
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_godot(
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

    writeln!(file, "extends Node")?;
    writeln!(file)?;
    writeln!(file, "signal play_note(pitch, duration)")?;
    writeln!(file)?;
    writeln!(file, "func _ready():")?;
    writeln!(file, "\tplay_pattern()")?;
    writeln!(file)?;
    writeln!(file, "func play_pattern():")?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_time = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let duration = end_time - start_time;

        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(
                file,
                "\tawait get_tree().create_timer({sleep_dur:.3}).timeout"
            )?;
            current_time = start_time;
        }

        let pitch = event.value;
        writeln!(file, "\tplay_note.emit({pitch:.3}, {duration:.3})")?;
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
    fn godot_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_sample_output_{}.gd", unique_temp_suffix()));
        export_sample_pattern_to_godot(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("extends Node"));
        assert!(content.contains("signal play_sample(sample_name, gain, pan, rate)"));
        assert!(content.contains("await get_tree().create_timer(1.000).timeout"));
        assert!(content.contains("play_sample.emit(\"bd\""));
        assert!(content.contains("play_sample.emit(\"sn\""));
    }

    #[test]
    fn godot_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_number_output_{}.gd", unique_temp_suffix()));
        export_number_pattern_to_godot(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("extends Node"));
        assert!(content.contains("signal play_note(pitch, duration)"));
        assert!(content.contains("await get_tree().create_timer(0.667).timeout"));
        assert!(content.contains("play_note.emit(60.000"));
        assert!(content.contains("play_note.emit(62.000"));
        assert!(content.contains("play_note.emit(64.000"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            export_sample_pattern_to_godot(pat, "test.gd", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            export_number_pattern_to_godot(pat, "test.gd", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
