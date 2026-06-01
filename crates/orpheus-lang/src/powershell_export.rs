//! The `powershell_export` module provides an exporter to `PowerShell` scripts.
//!
//! This exporter generates a `.ps1` script containing `[console]::beep()` commands
//! and `Start-Sleep` commands for evaluated Orpheus patterns.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
const SECONDS_PER_CYCLE: f64 = 2.0;

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn pitch_to_freq(pitch: f64) -> u32 {
    // Console::Beep needs an int between 37 and 32767
    let freq = 440.0 * ((pitch - 69.0) / 12.0).exp2();
    let freq = freq.round() as u32;
    freq.clamp(37, 32767)
}

/// Exports a sample pattern's evaluated events to a `PowerShell` script.
///
/// Note that samples don't map directly to console beeps, so this function
/// currently generates simple short beeps of different frequencies based on sample name.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub fn export_sample_pattern_to_powershell(
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

    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "# Orpheus PowerShell Export")?;
    writeln!(file, "$ErrorActionPreference = 'Stop'")?;

    let mut current_time = 0.0;

    for event in &events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let mut dur_time =
            (f64::from(event.part.end()) - f64::from(event.part.start())) * SECONDS_PER_CYCLE;

        let wait_time = start_time - current_time;
        if wait_time > 0.001 {
            let wait_ms = (wait_time * 1000.0).round() as u64;
            writeln!(file, "Start-Sleep -Milliseconds {wait_ms}")?;
            current_time += wait_time;
        }

        // Simple heuristic for frequency based on string length and first char
        let freq = 100
            + ((event.value.sample().len() as u32) * 50)
            + ((event.value.sample().chars().next().unwrap_or('a') as u32) % 10 * 20);
        let freq = freq.clamp(37, 32767);

        // Samples are often one-shots, cap duration to something reasonable if it's too long
        if dur_time > 0.5 {
            dur_time = 0.5;
        }

        let mut dur_ms = (dur_time * 1000.0).round() as u32;
        if dur_ms < 10 {
            dur_ms = 10;
        }

        writeln!(file, "[console]::beep({freq}, {dur_ms})")?;
        current_time += dur_time;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a `PowerShell` script.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
pub fn export_number_pattern_to_powershell(
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

    writeln!(file, "# Orpheus PowerShell Export")?;
    writeln!(file, "$ErrorActionPreference = 'Stop'")?;

    let mut current_time = 0.0;

    for event in &events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let dur_time =
            (f64::from(event.part.end()) - f64::from(event.part.start())) * SECONDS_PER_CYCLE;

        let wait_time = start_time - current_time;
        if wait_time > 0.001 {
            let wait_ms = (wait_time * 1000.0).round() as u64;
            writeln!(file, "Start-Sleep -Milliseconds {wait_ms}")?;
            current_time += wait_time;
        }

        // Value is treated as MIDI pitch (60 = Middle C = 261.63 Hz)
        let freq = pitch_to_freq(event.value);
        let mut dur_ms = (dur_time * 1000.0).round() as u32;

        if dur_ms < 10 {
            dur_ms = 10;
        }

        writeln!(file, "[console]::beep({freq}, {dur_ms})")?;
        current_time += dur_time;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn powershell_exporter_generates_sample_pattern() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_ps_sample.ps1");
        export_sample_pattern_to_powershell(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Orpheus PowerShell Export"));
        assert!(content.contains("[console]::beep"));
    }

    #[test]
    fn powershell_exporter_generates_number_pattern() {
        let source = "x = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_ps_number.ps1");
        export_number_pattern_to_powershell(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Orpheus PowerShell Export"));
        assert!(content.contains("[console]::beep"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_powershell(pat, "test.ps1", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module2 = eval_module("pat2 = 60 62", ReplMode::Loose).unwrap();
        let pat2 = module2.get("pat2").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_powershell(pat2, "test.ps1", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
