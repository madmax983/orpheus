//! The `gcode_export` module provides an exporter to 3D printer G-Code.
//!
//! This exporter translates evaluated number patterns (representing MIDI pitches)
//! into `G1` commands. By vibrating the stepper motors at specific feedrates
//! (speeds), we can "play" melodies on standard 3D printers like an Ender 3.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
const SECONDS_PER_CYCLE: f64 = 2.0;

// Base configuration for an Ender 3 style printer
// Steps per mm for X/Y axis is typically 80
const STEPS_PER_MM: f64 = 80.0;

/// Exports a number pattern's evaluated events to a G-Code file.
///
/// Number patterns are assumed to represent MIDI pitch. The exporter calculates
/// the required stepper motor feedrate to produce the corresponding frequency.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_gcode;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_music.gcode");
/// export_number_pattern_to_gcode(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_gcode(
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

    writeln!(file, "; Orpheus G-Code Music Export")?;
    writeln!(file, "; =========================")?;
    writeln!(file, "; Cycles: {cycle_count}")?;
    writeln!(file)?;

    // Setup printer
    writeln!(file, "G21 ; Set units to millimeters")?;
    writeln!(file, "G90 ; Use absolute coordinates")?;
    writeln!(file, "G28 X Y ; Home X and Y axes")?;
    writeln!(file, "G0 X100 Y100 Z10 F3000 ; Move to center")?;
    writeln!(file, "G91 ; Switch to relative positioning for movements")?;
    writeln!(file)?;

    let mut current_time = 0.0;
    // We alternate moving positive and negative to stay near the center
    let mut move_positive = true;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_time = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let duration_s = end_time - start_time;

        if start_time > current_time {
            let sleep_duration_seconds = start_time - current_time;
            // G4 P is in milliseconds
            let sleep_duration_milliseconds = sleep_duration_seconds * 1000.0;
            writeln!(file, "G4 P{sleep_duration_milliseconds:.0} ; Rest")?;
            // We do not need to update `current_time` here because it gets updated to `end_time` below.
            // current_time = start_time;
        }

        let pitch = event.value;
        // MIDI 69 is A4 (440Hz)
        let frequency_hz = 440.0 * ((pitch - 69.0) / 12.0).exp2();

        // Feedrate is mm/min.
        // frequency_hz = (feedrate / 60) * STEPS_PER_MM
        // feedrate = (frequency_hz * 60) / STEPS_PER_MM
        let feedrate_mm_per_min = (frequency_hz * 60.0) / STEPS_PER_MM;

        // Distance is speed * time
        // speed is mm/s = feedrate / 60
        let speed_mm_per_s = feedrate_mm_per_min / 60.0;
        let mut distance_mm = speed_mm_per_s * duration_s;

        if !move_positive {
            distance_mm = -distance_mm;
        }

        // We move both X and Y simultaneously to spread the wear and be louder
        writeln!(
            file,
            "G1 X{distance_mm:.3} Y{distance_mm:.3} F{feedrate_mm_per_min:.0} ; Note: {pitch}"
        )?;

        move_positive = !move_positive;
        current_time = end_time;
    }

    // Sleep remaining time of the sequence to allow looping if copied
    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * SECONDS_PER_CYCLE;
    if total_time > current_time {
        let sleep_ms = (total_time - current_time) * 1000.0;
        writeln!(file, "G4 P{sleep_ms:.0} ; Rest")?;
    }

    writeln!(file)?;
    writeln!(file, "G90 ; Switch back to absolute positioning")?;
    writeln!(file, "G28 X Y ; Home X and Y axes")?;
    writeln!(file, "M84 ; Disable motors")?;

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
    fn gcode_exporter_generates_gcode_for_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_gcode_output_{}.gcode", unique_temp_suffix()));
        export_number_pattern_to_gcode(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("; Orpheus G-Code Music Export"));
        // C4 (60) is ~261.63 Hz. Feedrate math will dictate exact F value.
        // F = (261.63 * 60) / 80 = 196.2225 -> 196
        assert!(content.contains("F196"));
        // D4 (62) is ~293.66 Hz -> F220
        assert!(content.contains("F220"));
        // E4 (64) is ~329.63 Hz -> F247
        assert!(content.contains("F247"));

        assert!(content.contains("G1 X"));
        assert!(content.contains('Y'));
    }

    #[test]
    fn gcode_exporter_handles_rests() {
        let source = "pattern = 60 ~ 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_gcode_output_{}.gcode", unique_temp_suffix()));
        export_number_pattern_to_gcode(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("; Orpheus G-Code Music Export"));
        // ~ will cause a rest G4 P...
        // The rest duration for a 1/3 cycle rest is 2.0 / 3.0 = 0.6666s = 667ms
        assert!(content.contains("G4 P667"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_gcode(pat, "test.gcode", 0)
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
    fn export_number_pattern_zero_cycles() {
        let source = "pattern = fast(2, 1 2)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_number.gcode");

        let err = export_number_pattern_to_gcode(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }
}
