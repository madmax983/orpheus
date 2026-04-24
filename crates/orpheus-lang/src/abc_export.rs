//! The `abc_export` module provides an exporter to ABC notation.
//!
//! This exporter generates a `.abc` text file containing a standard ABC
//! music notation document for an evaluated Orpheus number pattern.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

/// Maps a MIDI note number to an ABC notation pitch character.
#[must_use]
pub fn map_midi_to_abc(midi: i32) -> String {
    let note_names = [
        "C", "^C", "D", "^D", "E", "F", "^F", "G", "^G", "A", "^A", "B",
    ];
    let octave = (midi / 12) - 1;

    #[allow(clippy::cast_sign_loss)]
    let note_index = (midi % 12).rem_euclid(12) as usize;

    let base_name = note_names[note_index];

    match octave {
        o if o < 4 => {
            #[allow(clippy::cast_sign_loss)]
            let repeats = (4 - o) as usize;
            format!("{}{}", base_name, ",".repeat(repeats))
        }
        4 => base_name.to_string(), // Middle C octave
        5 => base_name.to_lowercase(),
        o if o > 5 => {
            #[allow(clippy::cast_sign_loss)]
            let repeats = (o - 5) as usize;
            format!("{}{}", base_name.to_lowercase(), "'".repeat(repeats))
        }
        _ => base_name.to_string(),
    }
}

/// Exports a number pattern's evaluated events to an ABC notation file.
///
/// Number patterns are assumed to represent MIDI pitch numbers, which are mapped
/// to their corresponding ABC pitch characters and durations based on Orpheus' cycle.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::abc_export::export_number_pattern_to_abc;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_abc.abc");
/// export_number_pattern_to_abc(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_abc(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    events.sort_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "X:1").map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(file, "T:Orpheus Export").map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(file, "M:4/4").map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(file, "L:1/4").map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(file, "K:C").map_err(|e| EvalError::new(e.to_string()))?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start());
        let end_time = f64::from(event.part.end());

        if start_time > current_time {
            let rest_duration = start_time - current_time;
            // Write a rest. In ABC, 'z' is a rest.
            // 4/4 time signature, L:1/4 means 1 unit = 1 quarter note.
            // Assuming 1 Orpheus cycle = 1 measure of 4/4 = 4 quarter notes.
            let rest_multiplier = rest_duration * 4.0;
            if rest_multiplier > 0.05 {
                write!(file, "z{rest_multiplier:.1} ").map_err(|e| EvalError::new(e.to_string()))?;
            }
        }

        #[allow(clippy::cast_possible_truncation)]
        let pitch = map_midi_to_abc(event.value.round() as i32);
        let note_duration = end_time - start_time;
        let note_multiplier = note_duration * 4.0;

        // Don't append '.0' to integer multipliers for cleaner output
        if (note_multiplier - note_multiplier.round()).abs() < f64::EPSILON {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let u64_multiplier = note_multiplier as u64;
            write!(file, "{pitch}{u64_multiplier} ").map_err(|e| EvalError::new(e.to_string()))?;
        } else {
            write!(file, "{pitch}{note_multiplier:.1} ").map_err(|e| EvalError::new(e.to_string()))?;
        }

        current_time = end_time;
    }

    writeln!(file, "|").map_err(|e| EvalError::new(e.to_string()))?;

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
    fn abc_exporter_generates_basic_c_major_scale() {
        let source = "pattern = 60 62 64 65"; // C4 D4 E4 F4
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_abc_output_{}.abc", unique_temp_suffix()));
        export_number_pattern_to_abc(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("X:1"));
        assert!(content.contains("T:Orpheus Export"));
        assert!(content.contains("M:4/4"));
        assert!(content.contains("L:1/4"));
        assert!(content.contains("K:C"));

        // 4 notes in 1 cycle means each note is 1 quarter note long, represented by duration 1.
        assert!(content.contains("C1 D1 E1 F1 |"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_abc(pat, "test.abc", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
