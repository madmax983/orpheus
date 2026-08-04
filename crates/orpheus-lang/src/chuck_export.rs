//! The `chuck_export` module provides an exporter to `ChucK` scripts.
//!
//! This exporter generates a `.ck` `ChucK` script containing shred routines
//! for evaluated Orpheus patterns, mapping Orpheus sample identifiers to
//! standard `ChucK` audio buffers, and playing number patterns as pitch information.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds = 2000 ms.
const MS_PER_CYCLE: f64 = 2000.0;

/// Maps a standard Orpheus sample name to its closest generic `ChucK` sample path.
#[must_use]
pub fn map_sample_to_chuck(sample: &str) -> &'static str {
    match sample {
        "bd" | "kick" => "data/kick.wav",
        "sn" | "snare" => "data/snare.wav",
        "hh" | "hat" => "data/hihat.wav",
        "oh" | "openhat" => "data/openhat.wav",
        "cp" | "clap" => "data/clap.wav",
        "cr" | "crash" => "data/crash.wav",
        "rd" | "ride" => "data/ride.wav",
        "tom" | "lt" => "data/tom_lo.wav",
        "mt" => "data/tom_mid.wav",
        "ht" => "data/tom_hi.wav",
        "bass" => "data/bass.wav",
        _ => "data/blip.wav",
    }
}

/// Exports a sample pattern's evaluated events to a `ChucK` script.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_sample_pattern_to_chuck;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.ck");
/// export_sample_pattern_to_chuck(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_chuck(
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
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "// Orpheus `ChucK` Export")?;
    writeln!(file, "// ======================")?;
    writeln!(file, "// Cycles: {cycle_count}")?;
    writeln!(file)?;

    writeln!(file, "fun void playOrpheusSamples() {{")?;
    writeln!(file, "    SndBuf buf => dac;")?;
    writeln!(file, "    0 => buf.gain;")?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * MS_PER_CYCLE;

        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(file, "    {sleep_dur}::ms => now;")?;
            current_time = start_time;
        }

        let ck_sample = map_sample_to_chuck(event.value.sample());
        let gain = event.value.gain();
        let rate = event.value.rate();

        writeln!(file, "    \"{ck_sample}\" => buf.read;")?;
        writeln!(file, "    {gain} => buf.gain;")?;
        writeln!(file, "    {rate} => buf.rate;")?;
        writeln!(file, "    0 => buf.pos;")?;
    }

    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * MS_PER_CYCLE;
    if total_time > current_time {
        writeln!(file, "    {}::ms => now;", total_time - current_time)?;
    }

    writeln!(file, "}}")?;
    writeln!(file)?;
    writeln!(file, "spork ~ playOrpheusSamples();")?;
    writeln!(file, "while(true) 1::second => now;")?;

    Ok(())
}

/// Exports a number pattern's evaluated events to a `ChucK` script.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_chuck;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_num.ck");
/// export_number_pattern_to_chuck(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_chuck(
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

    writeln!(file, "// Orpheus `ChucK` Export")?;
    writeln!(file, "// ======================")?;
    writeln!(file, "// Cycles: {cycle_count}")?;
    writeln!(file)?;

    writeln!(file, "fun void playOrpheusNotes() {{")?;
    writeln!(file, "    SinOsc osc => dac;")?;
    writeln!(file, "    0.5 => osc.gain;")?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * MS_PER_CYCLE;
        let end_time = f64::from(event.part.end()) * MS_PER_CYCLE;
        let duration = end_time - start_time;

        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(file, "    0.0 => osc.gain;")?;
            writeln!(file, "    {sleep_dur}::ms => now;")?;
            current_time = start_time;
        }

        let pitch = event.value;
        writeln!(file, "    Std.mtof({pitch}) => osc.freq;")?;
        writeln!(file, "    0.5 => osc.gain;")?;
        writeln!(file, "    {duration}::ms => now;")?;
        current_time += duration;
    }

    writeln!(file, "    0.0 => osc.gain;")?;

    #[allow(clippy::cast_precision_loss)]
    let total_time = (cycle_count as f64) * MS_PER_CYCLE;
    if total_time > current_time {
        writeln!(file, "    {}::ms => now;", total_time - current_time)?;
    }

    writeln!(file, "}}")?;
    writeln!(file)?;
    writeln!(file, "spork ~ playOrpheusNotes();")?;
    writeln!(file, "while(true) 1::second => now;")?;

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
    fn chuck_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_sample_output_{}.ck", unique_temp_suffix()));
        export_sample_pattern_to_chuck(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("// Orpheus `ChucK` Export"));
        assert!(content.contains("1000")); // ms wait
        assert!(content.contains("data/kick.wav"));
        assert!(content.contains("data/snare.wav"));
    }

    #[test]
    fn chuck_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_number_output_{}.ck", unique_temp_suffix()));
        export_number_pattern_to_chuck(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("// Orpheus `ChucK` Export"));
        assert!(content.contains("Std.mtof(60) => osc.freq;"));
        assert!(content.contains("Std.mtof(62) => osc.freq;"));
        assert!(content.contains("Std.mtof(64) => osc.freq;"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();
        let path = std::env::temp_dir().join(format!("zero_{}.ck", unique_temp_suffix()));
        assert_eq!(
            super::export_sample_pattern_to_chuck(pat, &path, 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join(format!("zero_num_{}.ck", unique_temp_suffix()));
        assert_eq!(
            super::export_number_pattern_to_chuck(pat, &path, 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
