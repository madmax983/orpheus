//! The `guitar_tab_export` module provides an exporter to ASCII guitar tablature.
//!
//! This exporter generates a `.tab` file containing standard 6-string guitar tablature
//! mapped from a sequence of evaluated number events representing MIDI notes.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

/// Standard guitar string tunings (MIDI notes).
const STRING_TUNINGS: [(&str, f64); 6] = [
    ("e", 64.0), // High E
    ("B", 59.0),
    ("G", 55.0),
    ("D", 50.0),
    ("A", 45.0),
    ("E", 40.0), // Low E
];

/// Exports a number pattern's evaluated events to an ASCII guitar tablature file.
///
/// Number patterns are assumed to represent MIDI pitch. Notes are mapped to the
/// most appropriate guitar string and fret.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_guitar_tab;
///
/// let env = eval_module("x = 40 45 50 55 59 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_tab.tab");
/// export_number_pattern_to_guitar_tab(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails, cycle count is zero, or if the file cannot be written.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn export_number_pattern_to_guitar_tab(
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

    // Resolution: 16 steps per cycle
    let steps_per_cycle = 16_u32;
    let total_steps = usize::try_from(cycle_count * u64::from(steps_per_cycle)).unwrap_or(0);

    if total_steps > 100_000 {
        return Err(EvalError::new(
            "evaluation exceeded the maximum allowed event limit",
        ));
    }

    // string_idx -> Vec of fret values over time
    // 0 is High E, 5 is Low E
    let mut grid: Vec<Vec<Option<u8>>> = vec![vec![None; total_steps]; 6];

    for event in events {
        let start_f64 = f64::from(event.part.start());
        let start_step = (start_f64 * f64::from(steps_per_cycle)).round() as usize;
        if start_step >= total_steps {
            continue;
        }

        let note = event.value;

        // Find the best string (lowest fret possible, but not negative)
        // We iterate from highest string to lowest.
        let mut best_string_idx = 5; // default to Low E
        let mut best_fret = 0;

        let mut found = false;
        for (idx, &(_, string_note)) in STRING_TUNINGS.iter().enumerate() {
            if note >= string_note {
                let fret = (note - string_note).round() as u8;
                if fret <= 24 {
                    // Standard max frets
                    best_string_idx = idx;
                    best_fret = fret;
                    found = true;
                    break;
                }
            }
        }

        if !found {
            if note < STRING_TUNINGS[5].1 {
                best_string_idx = 5;
                best_fret = 0; // Cap at open Low E if too low
            } else {
                let fret_f64 = note - STRING_TUNINGS[5].1;
                best_fret = fret_f64.round() as u8;
                best_string_idx = 5;
            }
        }

        grid[best_string_idx][start_step] = Some(best_fret);
    }

    writeln!(file, "Orpheus Guitar Tablature Export")?;
    writeln!(file, "Cycles: {cycle_count}, Resolution: 1/16")?;
    writeln!(file, "========================================")?;
    writeln!(file)?;

    // We can split into pages if total_steps is large, e.g. 16 steps per line
    let steps_per_line = 16;
    for chunk_start in (0..total_steps).step_by(steps_per_line) {
        let chunk_end = (chunk_start + steps_per_line).min(total_steps);

        for (idx, &(name, _)) in STRING_TUNINGS.iter().enumerate() {
            write!(file, "{name}|")?;
            for &fret_opt in grid[idx].iter().take(chunk_end).skip(chunk_start) {
                if let Some(fret) = fret_opt {
                    // Format fret, pad with dashes
                    let fret_str = fret.to_string();
                    write!(file, "{fret_str}")?;
                    if fret_str.len() == 1 {
                        write!(file, "-")?;
                    }
                } else {
                    write!(file, "--")?;
                }
                write!(file, "-")?; // extra separator
            }
            writeln!(file)?;
        }
        writeln!(file)?;
    }

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
    fn guitar_tab_exporter_generates_valid_tab() {
        let source = "pattern = 40 45 50 55 59 64"; // Open strings E A D G B e
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join(format!("test_output_{}.tab", unique_temp_suffix()));
        export_number_pattern_to_guitar_tab(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("e|"));
        assert!(content.contains("B|"));
        assert!(content.contains("G|"));
        assert!(content.contains("D|"));
        assert!(content.contains("A|"));
        assert!(content.contains("E|"));

        // 40 should map to E string fret 0.
        // Wait, 6 notes over 1 cycle means they land on steps:
        // 16 steps total. 0, 16/6, 32/6, 48/6, 64/6, 80/6.
        // Let's just check there's a 0 on the E string somewhere.
        assert!(content.contains("E|0-"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_guitar_tab(pat, "test.tab", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
