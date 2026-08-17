//! The `foxdot_export` module provides an exporter to `FoxDot` python scripts.
//!
//! This exporter generates a `.py` python script using `FoxDot`'s syntax,
//! translating Orpheus sample patterns and pitch patterns into `Player()` calls.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Maps an Orpheus sample name to its closest `FoxDot` equivalent string character.
/// `FoxDot` uses single character strings for its basic drum kit.
#[must_use]
pub fn map_sample_to_foxdot(sample: &str) -> char {
    match sample {
        "bd" | "kick" => 'x',
        "sn" | "snare" => 'o',
        "hh" | "hat" => '-',
        "oh" | "openhat" => '=',
        "cp" | "clap" => '*',
        "cr" | "crash" => '#',
        "ht" => '1',
        "mt" => '2',
        "lt" => '3',
        "noise" => 'n',
        _ => '~', // default to rest or a silent char in foxdot (space is rest)
    }
}

/// Exports a sample pattern's evaluated events to a `FoxDot` script.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_sample_pattern_to_foxdot};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.py");
/// export_sample_pattern_to_foxdot(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns an [`EvalError`] if evaluating the pattern over the time span fails.
pub fn export_sample_pattern_to_foxdot(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    // FoxDot expects events ordered by time
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "# Orpheus `FoxDot` Export")?;
    writeln!(
        file,
        "# Generated from {cycle_count} cycles of a sample pattern"
    )?;
    writeln!(file, "\nClock.bpm = 120\n")?;

    writeln!(file, "d1 >> play([")?;

    for event in &events {
        let sample = event.value.sample();
        let fx_char = map_sample_to_foxdot(sample);
        let start_time_beats = f64::from(event.part.start()) * 4.0; // Assuming 4 beats per cycle
        let dur_beats = (f64::from(event.part.end()) - f64::from(event.part.start())) * 4.0;

        writeln!(file, "    ('{fx_char}', {start_time_beats}, {dur_beats}),")?;
    }

    writeln!(file, "])")?;

    Ok(())
}

/// Exports a number pattern's evaluated events to a `FoxDot` script.
///
/// Number patterns are interpreted as pitch (MIDI notes or scale degrees).
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_foxdot};
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_num.py");
/// export_number_pattern_to_foxdot(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
///
/// Returns an [`EvalError`] if evaluating the pattern over the time span fails.
pub fn export_number_pattern_to_foxdot(
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
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "# Orpheus `FoxDot` Export")?;
    writeln!(
        file,
        "# Generated from {cycle_count} cycles of a number pattern"
    )?;
    writeln!(file, "\nClock.bpm = 120\n")?;

    writeln!(file, "p1 >> pluck([")?;

    for event in &events {
        let pitch = event.value;
        let start_time_beats = f64::from(event.part.start()) * 4.0;
        let dur_beats = (f64::from(event.part.end()) - f64::from(event.part.start())) * 4.0;

        writeln!(file, "    ({pitch}, {start_time_beats}, {dur_beats}),")?;
    }

    writeln!(file, "])")?;

    Ok(())
}
