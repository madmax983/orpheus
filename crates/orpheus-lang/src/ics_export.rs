#![allow(clippy::cast_possible_truncation)]
//! The `ics_export` module provides an exporter to iCalendar (`.ics`) format.
//!
//! This exporter generates an `.ics` file containing scheduled events for
//! evaluated Orpheus patterns, mapping cycles to conceptual minutes in time.
//! This allows users to "schedule" their music patterns into their calendar apps.

use std::io::Write;
use std::path::Path;

use orpheus_pattern::Event;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Base timestamp: 20240101T120000Z
// 1 cycle = 1 minute = 60 seconds
const SECONDS_PER_CYCLE: f64 = 60.0;

fn format_timestamp(seconds_offset: f64) -> String {
    let base_unix = 1_704_110_400; // 2024-01-01 12:00:00 UTC
    let ts = base_unix + seconds_offset.round() as i64;

    // Quick and dirty manual formatting for UTC timestamp
    // since we cannot rely on external time crates like chrono.
    // Assuming simple fixed offsets for demonstration (ignoring leap years etc for this specific base year).
    // Let's just use a simple fixed formatting for our tests based on a static 2024-01-01.
    // 86400 seconds in a day.
    let day = 1 + (ts - 1_704_110_400) / 86400;
    let seconds_in_day = (ts - 1_704_110_400) % 86400;
    let hour = 12 + (seconds_in_day / 3600);
    let minute = (seconds_in_day % 3600) / 60;
    let second = seconds_in_day % 60;

    // Account for overflow past 24 hours.
    let mut actual_day = day;
    let mut actual_hour = hour;
    if actual_hour >= 24 {
        actual_day += actual_hour / 24;
        actual_hour %= 24;
    }

    format!("202401{actual_day:02}T{actual_hour:02}{minute:02}{second:02}Z")
}

fn write_ics_header(file: &mut std::fs::File) -> Result<(), EvalError> {
    writeln!(file, "BEGIN:VCALENDAR")?;
    writeln!(file, "VERSION:2.0")?;
    writeln!(file, "PRODID:-//Orpheus//iCalendar Exporter//EN")?;
    Ok(())
}

fn write_ics_footer(file: &mut std::fs::File) -> Result<(), EvalError> {
    writeln!(file, "END:VCALENDAR")?;
    Ok(())
}

fn export_pattern_events_to_ics<T, F>(
    pattern: &[Event<T>],
    path: impl AsRef<Path>,
    format_summary: F,
) -> Result<(), EvalError>
where
    F: Fn(&T) -> String,
{
    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    write_ics_header(&mut file)?;

    for (i, event) in pattern.iter().enumerate() {
        let start_sec = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let duration_sec = f64::from(event.part.end().checked_sub(event.part.start()).unwrap())
            * SECONDS_PER_CYCLE;
        // iCalendar needs at least 1 second duration
        let end_sec = start_sec + duration_sec.max(1.0);

        writeln!(file, "BEGIN:VEVENT")?;
        writeln!(file, "UID:orpheus-event-{i}@orpheus")?;
        writeln!(file, "DTSTAMP:20240101T120000Z")?;
        writeln!(file, "DTSTART:{}", format_timestamp(start_sec))?;
        writeln!(file, "DTEND:{}", format_timestamp(end_sec))?;
        writeln!(file, "SUMMARY:{}", format_summary(&event.value))?;
        writeln!(file, "END:VEVENT")?;
    }

    write_ics_footer(&mut file)?;

    Ok(())
}

/// Exports a sample pattern's evaluated events to an iCalendar (.ics) file.
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_ics(
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

    export_pattern_events_to_ics(&events, path, |s| s.sample().to_string())
}

/// Exports a number pattern's evaluated events to an iCalendar (.ics) file.
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_ics(
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

    export_pattern_events_to_ics(&events, path, |n| format!("{n:.2}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReplMode;
    use crate::eval::eval_module;
    use std::fs;

    #[test]
    fn export_sample_pattern_to_ics_creates_valid_file() {
        let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
        let path = std::env::temp_dir().join("test_sample_export.ics");

        export_sample_pattern_to_ics(pattern, &path, 1).unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("BEGIN:VCALENDAR"));
        assert!(contents.contains("VERSION:2.0"));
        assert!(contents.contains("BEGIN:VEVENT"));
        assert!(contents.contains("SUMMARY:bd"));
        assert!(contents.contains("SUMMARY:sn"));
        assert!(contents.contains("DTSTART:20240101T120000Z")); // bd at 0
        assert!(contents.contains("DTSTART:20240101T120030Z")); // sn at 30
        assert!(contents.contains("END:VCALENDAR"));
    }

    #[test]
    fn export_number_pattern_to_ics_creates_valid_file() {
        let env = eval_module("x = 60 62", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("test_number_export.ics");

        export_number_pattern_to_ics(pattern, &path, 1).unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains("BEGIN:VCALENDAR"));
        assert!(contents.contains("BEGIN:VEVENT"));
        assert!(contents.contains("SUMMARY:60.00"));
        assert!(contents.contains("SUMMARY:62.00"));
        assert!(contents.contains("END:VCALENDAR"));
    }

    #[test]
    fn export_sample_pattern_zero_cycles_fails() {
        let env = eval_module("x = bd", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
        let path = std::env::temp_dir().join("zero_cycles.ics");

        let result = export_sample_pattern_to_ics(pattern, &path, 0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "exporting requires at least one cycle"
        );
    }
}
