//! The `minecraft_export` module provides an exporter to Minecraft Datapack scripts.
//!
//! This exporter generates a `.mcfunction` file containing `/playsound` commands
//! for evaluated Orpheus patterns, mapping Orpheus sample identifiers and number
//! patterns to standard Minecraft Note Block sounds and pitches.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds = 40 ticks
// Minecraft runs at 20 ticks per second.
const TICKS_PER_CYCLE: f64 = 40.0;

/// Maps a standard Orpheus sample name to its closest Minecraft Note Block sound.
#[must_use]
pub fn map_sample_to_minecraft(sample: &str) -> &'static str {
    match sample {
        "bd" | "kick" | "tom" | "lt" | "mt" | "ht" => "block.note_block.basedrum",
        "sn" | "snare" | "cp" | "clap" => "block.note_block.snare",
        "hh" | "hat" | "oh" | "openhat" => "block.note_block.hat",
        "bass" => "block.note_block.bass",
        _ => "block.note_block.harp",
    }
}

/// Converts a MIDI note number to a Minecraft pitch multiplier.
/// Minecraft note block pitch goes from 0.5 (F#3) to 2.0 (F#5), which corresponds
/// roughly to MIDI note 54 to 78.
#[must_use]
pub fn midi_to_minecraft_pitch(midi: f64) -> f64 {
    // MIDI 66 (F#4) is 1.0 multiplier.
    // 2.0 ^ ((midi - 66.0) / 12.0)
    ((midi - 66.0) / 12.0).exp2().clamp(0.5, 2.0)
}

/// Exports a sample pattern's evaluated events to a Minecraft .mcfunction script.
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_minecraft(
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

    writeln!(file, "# Orpheus Minecraft Export (Sample Pattern)")?;
    writeln!(file, "# ========================================")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(file)?;

    for event in events {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start_tick = (f64::from(event.part.start()) * TICKS_PER_CYCLE).round() as u32;
        let mc_sample = map_sample_to_minecraft(event.value.sample());
        let volume = event.value.gain().clamp(0.0, 1.0);

        if start_tick == 0 {
            writeln!(
                file,
                "playsound {mc_sample} master @a ~ ~ ~ {volume:.3} 1.0"
            )?;
        } else {
            writeln!(
                file,
                "schedule function orpheus_play_{} {}t",
                mc_sample.replace('.', "_"),
                start_tick
            )?;
            // Note: Generating the actual schedule functions requires a whole datapack structure,
            // this is a simplified representation leveraging schedule or just outputting raw commands.
            // For a single flat mcfunction, we just output the timing in comments and schedule.
            writeln!(
                file,
                "execute as @a at @s schedule function orpheus:play_{} {}t",
                mc_sample.replace('.', "_"),
                start_tick
            )?;
        }
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a Minecraft .mcfunction script.
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_minecraft(
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

    writeln!(file, "# Orpheus Minecraft Export (Number Pattern)")?;
    writeln!(file, "# ========================================")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(file)?;

    for event in events {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start_tick = (f64::from(event.part.start()) * TICKS_PER_CYCLE).round() as u32;
        let pitch = midi_to_minecraft_pitch(event.value);

        writeln!(file, "# tick: {start_tick}")?;
        if start_tick == 0 {
            writeln!(
                file,
                "playsound block.note_block.harp master @a ~ ~ ~ 1.0 {pitch:.3}"
            )?;
        } else {
            writeln!(
                file,
                "schedule function orpheus:play_harp_{pitch:.3} {start_tick}t"
            )?;
        }
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
    fn minecraft_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join(format!(
            "test_mc_sample_{}.mcfunction",
            unique_temp_suffix()
        ));
        export_sample_pattern_to_minecraft(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Orpheus Minecraft Export"));
        assert!(content.contains("playsound block.note_block.basedrum master @a"));
        assert!(content.contains("20t")); // sn should be at tick 20 (half a cycle)
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn minecraft_exporter_generates_number_pattern() {
        let source = "pattern = 66 78";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_mc_num_{}.mcfunction", unique_temp_suffix()));
        export_number_pattern_to_minecraft(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Orpheus Minecraft Export"));
        assert!(content.contains("playsound block.note_block.harp master @a ~ ~ ~ 1.0 1.000")); // pitch 1.0 for MIDI 66
        assert!(content.contains("2.000")); // pitch 2.0 for MIDI 78
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();
        assert_eq!(
            super::export_number_pattern_to_minecraft(pat, "test.mcfunction", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
