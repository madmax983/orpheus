//! The `abc_export` module provides an exporter to ABC musical notation.
//!
//! This exporter generates an `.abc` file containing musical notes
//! mapped from a number pattern, allowing rendering to standard sheet music.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
fn midi_to_abc(midi: f64) -> String {
    let midi_int = midi.round() as i32;
    let note_names = [
        "C", "^C", "D", "^D", "E", "F", "^F", "G", "^G", "A", "^A", "B",
    ];
    let octave = (midi_int / 12) - 1;
    let mut note_idx = midi_int % 12;
    if note_idx < 0 {
        note_idx += 12;
    }

    let base_note = note_names[note_idx as usize];

    match octave.cmp(&4) {
        std::cmp::Ordering::Equal => base_note.to_string(),
        std::cmp::Ordering::Greater => {
            let lower = base_note.to_lowercase();
            let marks = "'".repeat((octave - 4) as usize);
            format!("{lower}{marks}")
        }
        std::cmp::Ordering::Less => {
            let marks = ",".repeat((4 - octave).max(0) as usize);
            format!("{base_note}{marks}")
        }
    }
}

#[allow(clippy::cast_possible_truncation)]
/// Exports a number pattern's evaluated events to an ABC notation file.
///
/// Number patterns are assumed to represent MIDI note numbers.
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
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "X:1")?;
    writeln!(file, "T:Orpheus Export")?;
    writeln!(file, "M:4/4")?;
    writeln!(file, "L:1/16")?;
    writeln!(file, "K:C")?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start());
        let end_time = f64::from(event.part.end());
        let duration = end_time - start_time;

        if start_time > current_time {
            let rest_dur = start_time - current_time;
            #[allow(clippy::cast_possible_truncation)]
            let rest_16ths = (rest_dur * 16.0).round() as i32;
            if rest_16ths > 0 {
                write!(file, "z{rest_16ths} ")?;
            }
            current_time = start_time;
        }

        let abc_note = midi_to_abc(event.value);
        #[allow(clippy::cast_possible_truncation)]
        let dur_16ths = (duration * 16.0).round() as i32;
        if dur_16ths > 0 {
            write!(file, "{abc_note}{dur_16ths} ")?;
            current_time = end_time;
        }
    }

    writeln!(file)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};
    use std::fs;
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
    fn abc_exporter_generates_valid_format() {
        let source = "x = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_abc_output_{}.abc", unique_temp_suffix()));
        export_number_pattern_to_abc(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("X:1"));
        assert!(content.contains("T:Orpheus Export"));
        assert!(content.contains("C5 D5 E5 "));

        let _ = fs::remove_file(path);
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
