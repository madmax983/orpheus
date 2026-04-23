//! The `osu_export` module provides an exporter to osu!mania format.
//!
//! This exporter generates a `.osu` beatmap containing hit objects
//! for evaluated Orpheus patterns mapped to a 4K standard layout.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::SamplePatternValue;

/// Exports a sample pattern's evaluated events to an osu!mania `.osu` file.
///
/// The beatmap is generated with 4 columns (4K). Each sample name is mapped
/// heuristically to a specific column index (0 to 3) representing the x coordinate.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_sample_pattern_to_osu;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("beatmap.osu");
/// export_sample_pattern_to_osu(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn export_sample_pattern_to_osu(
    pattern: &SamplePatternValue,
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
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "osu file format v14")?;
    writeln!(file, "\n[General]")?;
    writeln!(file, "Mode: 3")?; // mania
    writeln!(file, "\n[Metadata]")?;
    writeln!(file, "Title:Orpheus Export")?;
    writeln!(file, "Artist:Nova")?;
    writeln!(file, "Creator:Orpheus")?;
    writeln!(file, "Version:1")?;
    writeln!(file, "\n[Difficulty]")?;
    writeln!(file, "CircleSize:4")?; // 4 keys (columns)

    writeln!(file, "\n[HitObjects]")?;

    let seconds_per_cycle = 2.0;

    for event in &events {
        let sample = event.value.sample();

        let col = match sample {
            "hh" | "hat" => 1,
            "sn" | "snare" | "cp" | "clap" => 2,
            "oh" | "openhat" | "cr" | "crash" => 3,
            _ => 0,
        };

        // x is computed by floor(x * columnCount / 512). Reverse: x = col * 128 + 64
        let x = col * 128 + 64;
        let y = 192; // Default for mania

        let time_ms = (f64::from(event.part.start()) * seconds_per_cycle * 1000.0).round() as u64;

        // hit circle: x,y,time,type,hitSound,hitSample
        // type 1 = hit circle
        // hitSound 0 = normal
        // hitSample 0:0:0:0: = default
        writeln!(file, "{x},{y},{time_ms},1,0,0:0:0:0:")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn osu_exporter_generates_valid_format() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_output.osu");
        export_sample_pattern_to_osu(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("osu file format v14"));
        assert!(content.contains("[HitObjects]"));
        assert!(content.contains("Mode: 3")); // mania mode
        assert!(content.contains("CircleSize:4")); // 4 keys

        // At 120bpm/2.0s cycles, bd starts at 0ms, sn starts at 1000ms
        // bd maps to col 0 -> x = 64
        // sn maps to col 2 -> x = 320
        assert!(content.contains("64,192,0,1,0,0:0:0:0:"));
        assert!(content.contains("320,192,1000,1,0,0:0:0:0:"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_osu(pat, "test.osu", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
