//! The `sonic_pi_export` module provides an exporter to Sonic Pi ruby scripts.
//!
//! This exporter generates a `.rb` ruby script containing `live_loop` blocks
//! for evaluated Orpheus patterns, mapping Orpheus sample identifiers to
//! standard Sonic Pi samples, and playing number patterns as pitch information.

use std::io::Write;
use std::path::Path;

use crate::eval::{Error, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
// Sonic Pi's default BPM is 60, so 1 sleep unit = 1 second.
// Therefore, 1 Orpheus cycle (2.0s) = 2.0 Sonic Pi sleep units.
const SLEEP_UNITS_PER_CYCLE: f64 = 2.0;

/// Maps a standard Orpheus sample name to its closest Sonic Pi equivalent.
#[must_use]
pub fn map_sample_to_sonic_pi(sample: &str) -> &'static str {
    match sample {
        "bd" | "kick" => ":bd_haus",
        "sn" | "snare" => ":sn_dolf",
        "hh" | "hat" => ":drum_cymbal_closed",
        "oh" | "openhat" => ":drum_cymbal_open",
        "cp" | "clap" => ":sn_generic",
        "cr" | "crash" => ":drum_cymbal_hard",
        "rd" | "ride" => ":drum_cymbal_soft",
        "tom" | "lt" => ":drum_tom_lo_soft",
        "mt" => ":drum_tom_mid_soft",
        "ht" => ":drum_tom_hi_soft",
        "bass" => ":bass_hit_c",
        _ => ":elec_pop",
    }
}

/// Exports a sample pattern's evaluated events to a Sonic Pi script.
///
/// Each line in the generated script represents a play or sample command
/// with its timing and parameters mapped to Sonic Pi.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::sonic_pi_export::export_sample_pattern_to_sonic_pi;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.rb");
/// export_sample_pattern_to_sonic_pi(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`Error`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_sonic_pi(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), crate::Error> {
    if cycle_count == 0 {
        return Err(Error::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    events.sort_unstable_by(|a, b| a.part.start().cmp(&b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| Error::new(e.to_string()))?;

    writeln!(file, "# Orpheus Sonic Pi Export")?;
    writeln!(file, "# =========================")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(file)?;

    writeln!(file, "live_loop :orpheus_samples do")?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SLEEP_UNITS_PER_CYCLE;

        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(file, "  sleep {sleep_dur:.3}")?;
            current_time = start_time;
        }

        let sp_sample = map_sample_to_sonic_pi(event.value.sample());
        let gain = event.value.gain();
        let pan = event.value.pan();
        let rate = event.value.rate();

        writeln!(file, "  sample {sp_sample}, amp: {gain:.3}, pan: {pan:.3}, rate: {rate:.3}")?;
    }

    // Sleep remaining time of the sequence to allow looping
    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * SLEEP_UNITS_PER_CYCLE;
    if total_time > current_time {
        writeln!(file, "  sleep {:.3}", total_time - current_time)?;
    }

    writeln!(file, "end")?;

    Ok(())
}

/// Exports a number pattern's evaluated events to a Sonic Pi script.
///
/// Number patterns are assumed to represent pitch, and map to `play` statements
/// in Sonic Pi.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::sonic_pi_export::export_number_pattern_to_sonic_pi;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_num.rb");
/// export_number_pattern_to_sonic_pi(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`Error`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_sonic_pi(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), crate::Error> {
    if cycle_count == 0 {
        return Err(Error::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    events.sort_unstable_by(|a, b| a.part.start().cmp(&b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| Error::new(e.to_string()))?;

    writeln!(file, "# Orpheus Sonic Pi Export")?;
    writeln!(file, "# =========================")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(file)?;

    writeln!(file, "live_loop :orpheus_notes do")?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SLEEP_UNITS_PER_CYCLE;
        let end_time = f64::from(event.part.end()) * SLEEP_UNITS_PER_CYCLE;
        let duration = end_time - start_time;

        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(file, "  sleep {sleep_dur:.3}")?;
            current_time = start_time;
        }

        let pitch = event.value;
        writeln!(file, "  play {pitch:.3}, release: {duration:.3}")?;
    }

    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * SLEEP_UNITS_PER_CYCLE;
    if total_time > current_time {
        writeln!(file, "  sleep {:.3}", total_time - current_time)?;
    }

    writeln!(file, "end")?;

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
    fn sonic_pi_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join(format!("test_sample_output_{}.rb", unique_temp_suffix()));
        export_sample_pattern_to_sonic_pi(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Orpheus Sonic Pi Export"));
        assert!(content.contains("sleep 1.000"));
        assert!(content.contains("sample :bd_haus"));
        assert!(content.contains("sample :sn_dolf"));
    }

    #[test]
    fn sonic_pi_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join(format!("test_number_output_{}.rb", unique_temp_suffix()));
        export_number_pattern_to_sonic_pi(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Orpheus Sonic Pi Export"));
        assert!(content.contains("sleep 0.667"));
        assert!(content.contains("play 60.000"));
        assert!(content.contains("play 62.000"));
        assert!(content.contains("play 64.000"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_sonic_pi(pat, "test.rb", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_sonic_pi(pat, "test.rb", 0)
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
        let path = std::env::temp_dir().join("test_zero_sample.rb");

        let err = export_sample_pattern_to_sonic_pi(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }

    #[test]
    fn export_number_pattern_zero_cycles() {
        let source = "pattern = fast(2, 1 2)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("test_zero_number.rb");

        let err = export_number_pattern_to_sonic_pi(pattern, &path, 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");
    }
}
