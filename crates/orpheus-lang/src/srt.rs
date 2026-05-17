//! The `srt` module provides a `SubRip` subtitle export format for evaluated patterns.
//!
//! This exporter generates a standard `.srt` subtitle file containing the events,
//! allowing pattern performances to be overlaid on videos or visualized in
//! standard media players.

use std::io::Write;
use std::path::Path;

use crate::eval::{Error, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to an SRT subtitle file.
///
/// Assumes a default tempo of 120 BPM (1 cycle = 2 seconds) for time mapping.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_sample_pattern_to_srt};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.srt");
/// export_sample_pattern_to_srt(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`Error`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_srt(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), crate::Error> {
    if cycle_count == 0 {
        return Err(Error::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    // SRT requires events to be sorted by start time
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    // Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
    let seconds_per_cycle = 2.0;

    for (index, event) in events.iter().enumerate() {
        let start_sec = f64::from(event.part.start()) * seconds_per_cycle;
        let end_sec = f64::from(event.part.end()) * seconds_per_cycle;

        let start_time = format_srt_time(start_sec);
        let end_time = format_srt_time(end_sec);

        writeln!(file, "{}", index + 1)?;
        writeln!(file, "{start_time} --> {end_time}")?;
        writeln!(file, "{}", event.value.sample())?;
        writeln!(file)?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to an SRT subtitle file.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_srt};
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("automation.srt");
/// export_number_pattern_to_srt(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`Error`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_srt(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), crate::Error> {
    if cycle_count == 0 {
        return Err(Error::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    let seconds_per_cycle = 2.0;

    for (index, event) in events.iter().enumerate() {
        let start_sec = f64::from(event.part.start()) * seconds_per_cycle;
        let end_sec = f64::from(event.part.end()) * seconds_per_cycle;

        let start_time = format_srt_time(start_sec);
        let end_time = format_srt_time(end_sec);

        writeln!(file, "{}", index + 1)?;
        writeln!(file, "{start_time} --> {end_time}")?;
        writeln!(file, "{:.3}", event.value)?;
        writeln!(file)?;
    }

    Ok(())
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn format_srt_time(seconds: f64) -> String {
    let millis = (seconds.fract() * 1000.0).round() as u32;
    let total_secs = seconds.trunc() as u32;
    let secs = total_secs % 60;
    let mins = (total_secs / 60) % 60;
    let hours = total_secs / 3600;
    format!("{hours:02}:{mins:02}:{secs:02},{millis:03}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn srt_exporter_generates_valid_format_for_samples() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_output.srt");
        export_sample_pattern_to_srt(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("1\n00:00:00,000 --> 00:00:01,000\nbd"));
        assert!(content.contains("2\n00:00:01,000 --> 00:00:02,000\nsn"));
    }

    #[test]
    fn srt_exporter_generates_valid_format_for_numbers() {
        let source = "x = 1 2";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_number_output.srt");
        export_number_pattern_to_srt(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("1\n00:00:00,000 --> 00:00:01,000\n1.000"));
        assert!(content.contains("2\n00:00:01,000 --> 00:00:02,000\n2.000"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_srt(pat, "test.srt", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_srt(pat, "test.srt", 0)
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
    fn export_sample_pattern_zero_cycles() {
        let source = "pattern = fast(2, bd sn)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_sample.srt");

        let err = export_sample_pattern_to_srt(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }

    #[test]
    fn export_number_pattern_zero_cycles() {
        let source = "pattern = fast(2, 1 2)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_number.srt");

        let err = export_number_pattern_to_srt(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }
}
