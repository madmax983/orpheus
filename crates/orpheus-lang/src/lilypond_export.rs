//! The `lilypond_export` module provides an exporter to `LilyPond` sheet music format.
//!
//! This exporter generates a `.ly` file containing notes for evaluated Orpheus
//! number patterns, mapping pitch values to `LilyPond` notation.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

/// Converts a MIDI note number into `LilyPond` pitch notation.
/// 60 is Middle C (c').
#[allow(clippy::cast_possible_truncation)]
fn midi_to_lilypond(midi_note: f64) -> String {
    let note = midi_note.round() as i32;
    let octave = (note / 12) - 1;
    let pc = note % 12;

    #[allow(clippy::match_same_arms)]
    let pc_str = match pc {
        0 => "c",
        1 => "cis",
        2 => "d",
        3 => "dis",
        4 => "e",
        5 => "f",
        6 => "fis",
        7 => "g",
        8 => "gis",
        9 => "a",
        10 => "ais",
        11 => "b",
        _ => "c",
    };

    #[allow(clippy::cast_sign_loss)]
    let octave_str = match octave.cmp(&4) {
        std::cmp::Ordering::Greater => "'".repeat((octave - 4) as usize),
        std::cmp::Ordering::Less => ",".repeat((4 - octave) as usize),
        std::cmp::Ordering::Equal => String::new(),
    };

    format!("{pc_str}{octave_str}")
}

/// Converts an Orpheus rational duration into a `LilyPond` duration string.
/// Assuming 1 cycle = 1 whole note (1).
#[allow(clippy::cast_possible_truncation)]
fn duration_to_lilypond(duration: f64) -> String {
    if duration <= 0.0 {
        return "4".to_string(); // fallback
    }

    let inv = (1.0 / duration).round() as i32;
    if inv == 1 {
        "1".to_string()
    } else if inv == 2 {
        "2".to_string()
    } else if inv > 2 && inv <= 4 {
        "4".to_string()
    } else if inv > 4 && inv <= 8 {
        "8".to_string()
    } else if inv > 8 && inv <= 16 {
        "16".to_string()
    } else if inv > 16 && inv <= 32 {
        "32".to_string()
    } else {
        "4".to_string() // fallback
    }
}

/// Exports a number pattern's evaluated events to a `LilyPond` `.ly` file.
///
/// Number patterns are assumed to represent pitch values (MIDI notes).
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_lilypond;
///
/// let env = eval_module("melody = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("melody").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.ly");
/// export_number_pattern_to_lilypond(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_lilypond(
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

    writeln!(file, "\\version \"2.24.1\"")?;
    writeln!(file, "\\header {{")?;
    writeln!(file, "  title = \"Orpheus Export\"")?;
    writeln!(file, "  composer = \"Nova\"")?;
    writeln!(file, "}}")?;
    writeln!(file)?;
    writeln!(file, "melody = {{")?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start());
        let end_time = f64::from(event.part.end());

        // Output rests if there is a gap
        if start_time > current_time + 0.001 {
            let rest_duration = start_time - current_time;
            let rest_str = duration_to_lilypond(rest_duration);
            write!(file, "r{rest_str} ")?;
        }

        let pitch_str = midi_to_lilypond(event.value);
        let duration = end_time - start_time;
        let dur_str = duration_to_lilypond(duration);

        write!(file, "{pitch_str}{dur_str} ")?;
        current_time = end_time;
    }

    writeln!(file, "\n}}")?;
    writeln!(file)?;
    writeln!(file, "\\score {{")?;
    writeln!(file, "  \\new Staff \\melody")?;
    writeln!(file, "  \\layout {{ }}")?;
    writeln!(file, "  \\midi {{ }}")?;
    writeln!(file, "}}")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};
    use std::fs;

    #[test]
    fn lilypond_exporter_generates_valid_format() {
        let source = "melody = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("melody").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_lilypond_output.ly");
        export_number_pattern_to_lilypond(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("\\version \"2.24.1\""));
        assert!(content.contains("melody = {"));
        assert!(content.contains("c4 d4 e4"));
        assert!(content.contains("\\new Staff \\melody"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("melody = 60 62", ReplMode::Loose).unwrap();
        let pattern = module.get("melody").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_lilypond(pattern, "test.ly", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
