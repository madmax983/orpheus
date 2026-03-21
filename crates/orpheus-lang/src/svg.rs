//! The `svg` module provides visual rendering of patterns as vector graphics.
//!
//! This module exports an evaluated `SamplePatternValue` into an SVG document,
//! visualizing the scheduled events as a traditional piano roll. This is highly
//! useful for visually debugging temporal structures and rhythmic intersections.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::SamplePatternValue;

/// Exports a sample pattern's evaluated events to an SVG file representing a piano roll.
///
/// # Panics
///
/// This function does not panic.
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_svg(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;
    let path = path.as_ref();

    let mut samples = BTreeSet::new();
    for event in &events {
        samples.insert(event.value.sample().to_string());
    }
    let sample_list: Vec<_> = samples.into_iter().collect();

    let lane_height = 40.0;
    let pixels_per_cycle = 200.0;
    #[allow(clippy::cast_precision_loss)]
    let width = (cycle_count as f64).mul_add(pixels_per_cycle, 100.0);
    #[allow(clippy::cast_precision_loss)]
    let height = (sample_list.len() as f64).mul_add(lane_height, 40.0);

    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(
        file,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" style=\"background-color: #1e1e1e; font-family: monospace;\">",
    )
    .map_err(|e| EvalError::new(e.to_string()))?;

    // Draw cycle lines
    for cycle in 0..=cycle_count {
        #[allow(clippy::cast_precision_loss)]
        let x = (cycle as f64).mul_add(pixels_per_cycle, 100.0);
        writeln!(
            file,
            "<line x1=\"{x}\" y1=\"0\" x2=\"{x}\" y2=\"{height}\" stroke=\"#333333\" stroke-width=\"2\" />",
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(
            file,
            "<text x=\"{x}\" y=\"20\" fill=\"#888888\" font-size=\"12\">Cycle {cycle}</text>",
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
    }

    // Draw lanes
    for (i, sample) in sample_list.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let y = (i as f64).mul_add(lane_height, 40.0);
        writeln!(
            file,
            "<text x=\"10\" y=\"{}\" fill=\"#ffffff\" font-size=\"14\">{sample}</text>",
            y + 25.0
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(
            file,
            "<line x1=\"100\" y1=\"{y}\" x2=\"{width}\" y2=\"{y}\" stroke=\"#333333\" stroke-width=\"1\" />",
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
    }

    // Draw events
    for event in events {
        let sample = event.value.sample().to_string();
        let lane_idx = sample_list.iter().position(|s| *s == sample).unwrap();
        #[allow(clippy::cast_precision_loss)]
        let y = (lane_idx as f64).mul_add(lane_height, 40.0) + 5.0;

        let start = f64::from(event.part.start());
        let end = f64::from(event.part.end());

        let x = start.mul_add(pixels_per_cycle, 100.0);
        let w = (end - start) * pixels_per_cycle;

        writeln!(
            file,
            "<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{}\" fill=\"#4CAF50\" rx=\"4\" opacity=\"{}\" />",
            lane_height - 10.0,
            event.value.gain().max(0.1)
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
    }

    writeln!(file, "</svg>").map_err(|e| EvalError::new(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn svg_exporter_generates_piano_roll() {
        let source = "pattern = fast(2, bd sn)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_output.svg");
        export_sample_pattern_to_svg(pattern, &path, 2).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<svg"));
        assert!(content.contains("bd"));
        assert!(content.contains("sn"));
        assert!(content.contains("<rect"));
    }
}
