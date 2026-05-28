//! The `csound_export` module provides an exporter to Csound (`.csd`) scripts.
//!
//! This exporter generates a `.csd` script containing `instr` blocks
//! for evaluated Orpheus patterns, mapping Orpheus sample identifiers to
//! strings/macros, and playing number patterns as pitch information using
//! conceptual Csound instruments.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a sample pattern's evaluated events to a Csound script.
///
/// Each event translates into an `i` statement (score event) in Csound.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_sample_pattern_to_csound;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.csd");
/// export_sample_pattern_to_csound(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_csound(
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

    writeln!(file, "<CsoundSynthesizer>")?;
    writeln!(file, "<CsOptions>")?;
    writeln!(file, "-odac")?;
    writeln!(file, "</CsOptions>")?;
    writeln!(file, "<CsInstruments>")?;
    writeln!(file, "sr = 44100")?;
    writeln!(file, "ksmps = 32")?;
    writeln!(file, "nchnls = 2")?;
    writeln!(file, "0dbfs = 1.0")?;
    writeln!(file)?;
    writeln!(file, "instr 1 ; Conceptual Sample Player")?;
    writeln!(file, "  Ssample = p4")?;
    writeln!(file, "  kamp = p5")?;
    writeln!(file, "  kpan = p6")?;
    writeln!(file, "  krate = p7")?;
    writeln!(file, "  ; Implementation would load and play Ssample")?;
    writeln!(file, "  a1 = 0")?;
    writeln!(file, "  a2 = 0")?;
    writeln!(file, "  outs a1 * kamp, a2 * kamp")?;
    writeln!(file, "endin")?;
    writeln!(file, "</CsInstruments>")?;
    writeln!(file, "<CsScore>")?;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_time = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let duration = end_time - start_time;

        let sample = event.value.sample();
        let gain = event.value.gain();
        let pan = event.value.pan();
        let rate = event.value.rate();

        writeln!(
            file,
            "i 1 {start_time:.3} {duration:.3} \"{sample}\" {gain:.3} {pan:.3} {rate:.3}"
        )?;
    }

    writeln!(file, "e")?;
    writeln!(file, "</CsScore>")?;
    writeln!(file, "</CsoundSynthesizer>")?;

    Ok(())
}

/// Exports a number pattern's evaluated events to a Csound script.
///
/// Number patterns are assumed to represent pitch (MIDI), and map to
/// score events in Csound.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_csound;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_num.csd");
/// export_number_pattern_to_csound(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_csound(
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

    writeln!(file, "<CsoundSynthesizer>")?;
    writeln!(file, "<CsOptions>")?;
    writeln!(file, "-odac")?;
    writeln!(file, "</CsOptions>")?;
    writeln!(file, "<CsInstruments>")?;
    writeln!(file, "sr = 44100")?;
    writeln!(file, "ksmps = 32")?;
    writeln!(file, "nchnls = 2")?;
    writeln!(file, "0dbfs = 1.0")?;
    writeln!(file)?;
    writeln!(file, "instr 1 ; Conceptual Synth")?;
    writeln!(file, "  imidi = p4")?;
    writeln!(file, "  kamp = p5")?;
    writeln!(file, "  ifreq = cpsmidinn(imidi)")?;
    writeln!(file, "  a1 oscil kamp, ifreq")?;
    writeln!(file, "  outs a1, a1")?;
    writeln!(file, "endin")?;
    writeln!(file, "</CsInstruments>")?;
    writeln!(file, "<CsScore>")?;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let end_time = f64::from(event.part.end()) * SECONDS_PER_CYCLE;
        let duration = end_time - start_time;

        let pitch = event.value;
        writeln!(file, "i 1 {start_time:.3} {duration:.3} {pitch:.3} 0.5")?;
    }

    writeln!(file, "e")?;
    writeln!(file, "</CsScore>")?;
    writeln!(file, "</CsoundSynthesizer>")?;

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
    fn csound_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_sample_output_{}.csd", unique_temp_suffix()));
        export_sample_pattern_to_csound(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<CsoundSynthesizer>"));
        assert!(content.contains("<CsScore>"));
        assert!(content.contains("i 1 0.000 1.000 \"bd\""));
        assert!(content.contains("i 1 1.000 1.000 \"sn\""));
    }

    #[test]
    fn csound_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_number_output_{}.csd", unique_temp_suffix()));
        export_number_pattern_to_csound(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<CsoundSynthesizer>"));
        assert!(content.contains("<CsScore>"));
        assert!(content.contains("i 1 0.000 0.667 60.000 0.5"));
        assert!(content.contains("i 1 0.667 0.667 62.000 0.5"));
        assert!(content.contains("i 1 1.333 0.667 64.000 0.5"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_csound(pat, "test.csd", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_csound(pat, "test.csd", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
