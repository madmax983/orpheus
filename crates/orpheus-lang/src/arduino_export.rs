//! The `arduino_export` module provides an exporter to Arduino C++ sketches.
//!
//! This exporter generates an `.ino` Arduino sketch containing arrays of
//! pitch frequencies and durations, intended to be played using the standard
//! Arduino `tone()` function on a piezo buzzer.

use std::io::Write;
use std::path::Path;

use crate::eval::{Error, render_span};
use crate::value::NumberPatternValue;

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds = 2000 ms.
const MS_PER_CYCLE: f64 = 2000.0;

/// Converts a MIDI note number to its corresponding frequency in Hertz.
#[must_use]
pub fn midi_to_hz(midi: f64) -> f64 {
    440.0 * ((midi - 69.0) / 12.0).exp2()
}

/// Exports a number pattern's evaluated events to an Arduino sketch.
///
/// Number patterns are assumed to represent pitch, and map to `tone()` calls
/// in the Arduino environment.
///
/// # Examples
///
/// ```no_run
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_arduino;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_num.ino");
/// export_number_pattern_to_arduino(pattern, &path, 2, 8).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`Error`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_arduino(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
    pin: u8,
) -> Result<(), crate::Error> {
    if cycle_count == 0 {
        return Err(Error::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    events.sort_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| Error::new(e.to_string()))?;

    writeln!(file, "// Orpheus Arduino Export")?;
    writeln!(file, "// =======================")?;
    writeln!(file, "// Cycles: {cycle_count}")?;
    writeln!(file, "// Note: Connect a piezo buzzer to PIN {pin}")?;
    writeln!(file)?;

    writeln!(file, "const int BUZZER_PIN = {pin};")?;
    writeln!(file)?;

    writeln!(file, "struct Note {{")?;
    writeln!(file, "  unsigned int frequency;")?;
    writeln!(file, "  unsigned long duration_ms;")?;
    writeln!(file, "  unsigned long sleep_before_ms;")?;
    writeln!(file, "}};")?;
    writeln!(file)?;

    writeln!(file, "const int SEQUENCE_LENGTH = {};", events.len())?;
    writeln!(file, "Note sequence[SEQUENCE_LENGTH] = {{")?;

    let mut current_time_ms = 0.0;

    for (i, event) in events.iter().enumerate() {
        let start_ms = f64::from(event.part.start()) * MS_PER_CYCLE;
        let end_ms = f64::from(event.part.end()) * MS_PER_CYCLE;
        let mut duration_ms = end_ms - start_ms;

        if duration_ms < 1.0 {
            duration_ms = 100.0; // default 100ms
        }

        let sleep_before_ms = if start_ms > current_time_ms {
            start_ms - current_time_ms
        } else {
            0.0
        };
        current_time_ms = start_ms + duration_ms;

        let freq = midi_to_hz(event.value);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let freq_int = freq.round() as u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let dur_int = duration_ms.round() as u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sleep_int = sleep_before_ms.round() as u32;

        let comma = if i < events.len() - 1 { "," } else { "" };
        writeln!(file, "  {{ {freq_int}, {dur_int}, {sleep_int} }}{comma}")?;
    }

    writeln!(file, "}};")?;
    writeln!(file)?;

    #[allow(clippy::cast_precision_loss)]
    let total_time_ms = (cycle_count as f64) * MS_PER_CYCLE;
    let mut final_sleep_int = 0;
    if total_time_ms > current_time_ms {
        let final_sleep = total_time_ms - current_time_ms;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            final_sleep_int = final_sleep.round() as u32;
        }
    }

    writeln!(file, "void setup() {{")?;
    writeln!(file, "  pinMode(BUZZER_PIN, OUTPUT);")?;
    writeln!(file, "}}")?;
    writeln!(file)?;

    writeln!(file, "void loop() {{")?;
    writeln!(file, "  for (int i = 0; i < SEQUENCE_LENGTH; i++) {{")?;
    writeln!(file, "    if (sequence[i].sleep_before_ms > 0) {{")?;
    writeln!(file, "      delay(sequence[i].sleep_before_ms);")?;
    writeln!(file, "    }}")?;
    writeln!(
        file,
        "    tone(BUZZER_PIN, sequence[i].frequency, sequence[i].duration_ms);"
    )?;
    writeln!(file, "    delay(sequence[i].duration_ms);")?;
    writeln!(file, "  }}")?;
    if final_sleep_int > 0 {
        writeln!(file, "  delay({final_sleep_int});")?;
    }
    writeln!(file, "}}")?;

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
    fn arduino_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_arduino_output_{}.ino", unique_temp_suffix()));
        export_number_pattern_to_arduino(pattern, &path, 1, 8).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("// Orpheus Arduino Export"));
        assert!(content.contains("const int BUZZER_PIN = 8;"));
        assert!(content.contains("{ 262, 667, 0 }"));
        assert!(content.contains("tone(BUZZER_PIN"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_arduino(pat, "test.ino", 0, 8)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
