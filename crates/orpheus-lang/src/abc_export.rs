//! Export functions for rendering Orpheus patterns to ABC notation.
//!
//! This module provides functionality to convert `NumberPatternValue` (representing
//! pitch sequences) into `.abc` files. ABC notation is a text-based music format
//! widely used for sharing melodies and rendering sheet music.

use std::io::Write;
use std::path::Path;

use orpheus_pattern::Rational;

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

/// Converts a MIDI note number (0-127) to an ABC pitch character string.
/// Middle C (MIDI 60) maps to "C".
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn midi_to_abc(midi_note: f64) -> String {
    let note = midi_note.round().clamp(0.0, 127.0) as u8;
    let octave = note / 12;
    let pitch_class = note % 12;

    let (base_char, accidental) = match pitch_class {
        0 => ("C", ""),
        1 => ("C", "^"),
        2 => ("D", ""),
        3 => ("D", "^"),
        4 => ("E", ""),
        5 => ("F", ""),
        6 => ("F", "^"),
        7 => ("G", ""),
        8 => ("G", "^"),
        9 => ("A", ""),
        10 => ("A", "^"),
        11 => ("B", ""),
        _ => unreachable!(),
    };

    // In ABC notation:
    // C, (octave 3) is MIDI 36
    // C (octave 4) is MIDI 48
    // c (octave 5) is MIDI 60 (Middle C)
    // c' (octave 6) is MIDI 72

    let octave_modifier = match octave.cmp(&4) {
        std::cmp::Ordering::Less => ",".repeat((4 - octave).into()),
        std::cmp::Ordering::Greater => {
            if octave > 5 {
                "'".repeat((octave - 5).into())
            } else {
                String::new()
            }
        }
        std::cmp::Ordering::Equal => String::new(),
    };

    let base = if octave >= 5 {
        base_char.to_lowercase()
    } else {
        base_char.to_string()
    };

    format!("{accidental}{base}{octave_modifier}")
}

fn rational_to_abc_duration(duration: Rational) -> String {
    // In our export, we'll set default note length (L:) to 1 cycle (whole note = 1).
    // So duration just becomes the fraction.
    // If it's exactly 1/1, we can return empty or "1".
    if duration.numerator() == 1 && duration.denominator() == 1 {
        return String::new(); // default length
    }
    format!("{}/{}", duration.numerator(), duration.denominator())
}

/// Exports a number pattern's evaluated events to ABC notation.
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails, the cycle count is 0,
/// or if the file cannot be written to disk.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_abc};
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.abc");
/// export_number_pattern_to_abc(pattern, &path, 1).unwrap();
/// ```
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

    // Sort events by start time
    events.sort_by_key(|e| *e.part.start());

    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "X:1")
        .and_then(|()| writeln!(file, "T:Orpheus Export"))
        .and_then(|()| writeln!(file, "M:4/4"))
        .and_then(|()| writeln!(file, "L:1/1")) // Base length is 1 cycle
        .and_then(|()| writeln!(file, "K:C"))
        .map_err(|e| EvalError::new(e.to_string()))?;

    let mut current_time = orpheus_pattern::Rational::zero();

    for event in events {
        // Output rest if there's a gap
        if *event.part.start() > current_time {
            let rest_dur = event
                .part
                .start()
                .checked_sub(&current_time)
                .map_err(|e| EvalError::new(e.to_string()))?;
            write!(file, "z{} ", rational_to_abc_duration(rest_dur))
                .map_err(|e| EvalError::new(e.to_string()))?;
        }

        let abc_note = midi_to_abc(event.value);
        let note_dur = event
            .part
            .end()
            .checked_sub(event.part.start())
            .map_err(|e| EvalError::new(e.to_string()))?;
        write!(file, "{abc_note}{} ", rational_to_abc_duration(note_dur))
            .map_err(|e| EvalError::new(e.to_string()))?;

        current_time = *event.part.end();
    }

    writeln!(file).map_err(|e| EvalError::new(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};
    use std::fs;

    #[test]
    fn midi_to_abc_translates_correctly() {
        assert_eq!(midi_to_abc(60.0), "c");
        assert_eq!(midi_to_abc(61.0), "^c");
        assert_eq!(midi_to_abc(62.0), "d");
        assert_eq!(midi_to_abc(72.0), "c'");
        assert_eq!(midi_to_abc(48.0), "C");
        assert_eq!(midi_to_abc(36.0), "C,");
    }

    #[test]
    fn abc_export_handles_durations() {
        let env = eval_module("x = 60 62", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_durations.abc");
        export_number_pattern_to_abc(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("c1/2 d1/2 "));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn abc_export_handles_rests() {
        let env = eval_module("x = 60 ~ 62", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_rests.abc");
        export_number_pattern_to_abc(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("c1/3 z1/3 d1/3 "));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn abc_export_boundary_condition() {
        let env = eval_module("x = 60", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("test_boundary.abc");

        let err = export_number_pattern_to_abc(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }
}
