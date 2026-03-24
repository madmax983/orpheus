//! The `html` module provides visual rendering of patterns as interactive HTML documents.
//!
//! This module exports evaluated patterns into an HTML document,
//! visualizing the scheduled events using web technologies.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to an HTML file.
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
///
/// # Panics
///
/// Panics if an event's sample is not found in the pre-computed sample list.
pub fn export_sample_pattern_to_html(
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

    let mut file = std::fs::File::create(path)?;
    writeln!(
        file,
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"UTF-8\">\n<title>Orpheus Pattern Export</title>\n<style>\nbody {{ background-color: #1e1e1e; color: #ffffff; font-family: monospace; padding: 20px; }}\n.event {{ position: absolute; background-color: #4CAF50; border-radius: 4px; box-sizing: border-box; border: 1px solid #388E3C; cursor: pointer; transition: transform 0.1s; }}\n.event:hover {{ transform: scale(1.02); z-index: 10; }}\n.lane {{ position: absolute; border-bottom: 1px solid #333333; left: 100px; right: 0; }}\n.lane-label {{ position: absolute; left: 10px; font-size: 14px; }}\n.cycle-line {{ position: absolute; top: 0; bottom: 0; border-left: 2px solid #333333; }}\n.cycle-label {{ position: absolute; top: 5px; color: #888888; font-size: 12px; transform: translateX(-50%); }}\n#container {{ position: relative; width: {width}px; height: {height}px; background-color: #252525; overflow: hidden; border: 1px solid #444; margin-top: 20px; }}\n</style>\n</head>\n<body>\n<h2>Orpheus Sample Pattern</h2>\n<div id=\"container\">"
    )?;

    // Draw cycle lines
    for cycle in 0..=cycle_count {
        #[allow(clippy::cast_precision_loss)]
        let x = (cycle as f64).mul_add(pixels_per_cycle, 100.0);
        writeln!(
            file,
            "<div class=\"cycle-line\" style=\"left: {x}px;\"></div>\n<div class=\"cycle-label\" style=\"left: {x}px;\">Cycle {cycle}</div>"
        )?;
    }

    // Draw lanes
    for (i, sample) in sample_list.iter().enumerate() {
        #[allow(clippy::cast_precision_loss)]
        let y = (i as f64).mul_add(lane_height, 40.0);
        writeln!(
            file,
            "<div class=\"lane-label\" style=\"top: {}px;\">{sample}</div>\n<div class=\"lane\" style=\"top: {}px; height: {}px;\"></div>",
            y + 12.0,
            y,
            lane_height
        )?;
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
            "<div class=\"event\" style=\"left: {x}px; top: {y}px; width: {w}px; height: {}px; opacity: {};\" title=\"Sample: {sample}&#10;Start: {:.3}&#10;End: {:.3}&#10;Gain: {:.2}\"></div>",
            lane_height - 10.0,
            event.value.gain().max(0.1),
            start,
            end,
            event.value.gain()
        )?;
    }

    writeln!(file, "</div>\n</body>\n</html>")?;

    Ok(())
}

/// Exports a number pattern's evaluated events to an HTML file.
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_html(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;
    let path = path.as_ref();

    let mut min_val = f64::INFINITY;
    let mut max_val = f64::NEG_INFINITY;
    for event in &events {
        if event.value < min_val {
            min_val = event.value;
        }
        if event.value > max_val {
            max_val = event.value;
        }
    }

    if min_val.is_infinite() || max_val.is_infinite() {
        min_val = 0.0;
        max_val = 1.0;
    } else if (max_val - min_val).abs() < f64::EPSILON {
        min_val -= 0.5;
        max_val += 0.5;
    }

    let pixels_per_cycle = 200.0;
    let height = 150.0;
    let margin = 40.0;
    #[allow(clippy::cast_precision_loss)]
    let width = (cycle_count as f64).mul_add(pixels_per_cycle, 100.0);
    let total_height = 2.0_f64.mul_add(margin, height);

    let mut file = std::fs::File::create(path)?;
    writeln!(
        file,
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"UTF-8\">\n<title>Orpheus Number Pattern Export</title>\n<style>\nbody {{ background-color: #1e1e1e; color: #ffffff; font-family: monospace; padding: 20px; }}\n.event {{ position: absolute; background-color: #2196F3; border-radius: 2px; cursor: pointer; transition: background-color 0.1s; }}\n.event:hover {{ background-color: #64B5F6; z-index: 10; }}\n.axis-line {{ position: absolute; border-bottom: 1px dashed #555555; left: 100px; right: 0; }}\n.axis-label {{ position: absolute; left: 10px; font-size: 14px; }}\n.cycle-line {{ position: absolute; top: 0; bottom: 0; border-left: 2px solid #333333; }}\n.cycle-label {{ position: absolute; top: 5px; color: #888888; font-size: 12px; transform: translateX(-50%); }}\n#container {{ position: relative; width: {width}px; height: {total_height}px; background-color: #252525; overflow: hidden; border: 1px solid #444; margin-top: 20px; }}\n</style>\n</head>\n<body>\n<h2>Orpheus Number Pattern</h2>\n<div id=\"container\">"
    )?;

    // Draw cycle lines
    for cycle in 0..=cycle_count {
        #[allow(clippy::cast_precision_loss)]
        let x = (cycle as f64).mul_add(pixels_per_cycle, 100.0);
        writeln!(
            file,
            "<div class=\"cycle-line\" style=\"left: {x}px;\"></div>\n<div class=\"cycle-label\" style=\"left: {x}px;\">Cycle {cycle}</div>"
        )?;
    }

    // Draw min/max labels and axis lines
    writeln!(
        file,
        "<div class=\"axis-label\" style=\"top: {}px;\">{:.2}</div>\n<div class=\"axis-line\" style=\"top: {}px;\"></div>",
        margin - 7.0,
        max_val,
        margin
    )?;
    writeln!(
        file,
        "<div class=\"axis-label\" style=\"top: {}px;\">{:.2}</div>\n<div class=\"axis-line\" style=\"top: {}px;\"></div>",
        margin + height - 7.0,
        min_val,
        margin + height
    )?;

    // Draw events
    for event in events {
        let start = f64::from(event.part.start());
        let end = f64::from(event.part.end());

        let x = start.mul_add(pixels_per_cycle, 100.0);
        let w = (end - start) * pixels_per_cycle;

        let normalized_val = (event.value - min_val) / (max_val - min_val);
        let y = normalized_val.mul_add(-height, margin + height);

        writeln!(
            file,
            "<div class=\"event\" style=\"left: {x}px; top: {}px; width: {w}px; height: 4px;\" title=\"Value: {:.3}&#10;Start: {:.3}&#10;End: {:.3}\"></div>",
            y - 2.0,
            event.value,
            start,
            end
        )?;
    }

    writeln!(file, "</div>\n</body>\n</html>")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn html_exporter_generates_sample_pattern() {
        let source = "pattern = fast(2, bd sn)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_output.html");
        export_sample_pattern_to_html(pattern, &path, 2).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("bd"));
        assert!(content.contains("sn"));
        assert!(content.contains("class=\"event\""));
    }

    #[test]
    fn html_exporter_generates_number_pattern() {
        let source = "pattern = 1 2 3";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_number_output.html");
        export_number_pattern_to_html(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<!DOCTYPE html>"));
        assert!(content.contains("1.00"));
        assert!(content.contains("3.00"));
        assert!(content.contains("class=\"event\""));
    }
}
