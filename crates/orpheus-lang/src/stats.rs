//! The `stats` module provides analytical utilities for evaluated patterns.
//!
//! This module implements the core logic for the `:stats` REPL command, calculating
//! metadata such as total event count, unique samples, density, and numerical ranges
//! from a concrete pattern over a specified number of cycles.

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Analyzes a sample pattern's evaluated events and returns a formatted report.
///
/// The report contains the total number of events, unique samples triggered,
/// and the event density (events per cycle).
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if `cycle_count` is 0.
pub fn sample_pattern_stats(
    binding_name: &str,
    pattern: &SamplePatternValue,
    cycle_count: u64,
) -> Result<String, EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("stats requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;

    let total_events = events.len();
    let mut samples = BTreeSet::new();
    for event in &events {
        samples.insert(event.value.sample().to_string());
    }

    let unique_count = samples.len();
    let sample_list = samples.into_iter().collect::<Vec<_>>().join(", ");
    #[allow(clippy::cast_precision_loss)]
    let density = (total_events as f64) / (cycle_count as f64);

    let rows = [
        ("Total Events", total_events.to_string()),
        ("Unique Samples", format!("{unique_count} ({sample_list})")),
        ("Event Density", format!("{density:.2} events/cycle")),
    ];

    let title = format!("Pattern Stats: {binding_name} ({cycle_count} cycles)");
    Ok(format_stats_table(&title, &rows))
}

/// Analyzes a number pattern's evaluated events and returns a formatted report.
///
/// The report contains the total number of events, minimum value, maximum value,
/// average value, and the event density (events per cycle).
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if `cycle_count` is 0.
pub fn number_pattern_stats(
    binding_name: &str,
    pattern: &NumberPatternValue,
    cycle_count: u64,
) -> Result<String, EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("stats requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;

    let total_events = events.len();
    let mut min_val = f64::INFINITY;
    let mut max_val = f64::NEG_INFINITY;
    let mut sum = 0.0;

    for event in &events {
        let val = event.value;
        if val < min_val {
            min_val = val;
        }
        if val > max_val {
            max_val = val;
        }
        sum += val;
    }

    let avg = if total_events > 0 {
        #[allow(clippy::cast_precision_loss)]
        let total = total_events as f64;
        sum / total
    } else {
        0.0
    };

    if min_val.is_infinite() {
        min_val = 0.0;
    }
    if max_val.is_infinite() {
        max_val = 0.0;
    }

    #[allow(clippy::cast_precision_loss)]
    let density = (total_events as f64) / (cycle_count as f64);

    let rows = [
        ("Total Events", total_events.to_string()),
        ("Min Value", format!("{min_val:.3}")),
        ("Max Value", format!("{max_val:.3}")),
        ("Average Value", format!("{avg:.3}")),
        ("Event Density", format!("{density:.2} events/cycle")),
    ];

    let title = format!("Pattern Stats: {binding_name} ({cycle_count} cycles)");
    Ok(format_stats_table(&title, &rows))
}

fn format_stats_table(title: &str, rows: &[(&str, String)]) -> String {
    let mut max_key_len = 0;
    let mut max_val_len = 0;
    for (key, val) in rows {
        if key.len() > max_key_len {
            max_key_len = key.len();
        }
        if val.len() > max_val_len {
            max_val_len = val.len();
        }
    }

    let min_content_width = max_key_len + 3 + max_val_len;
    let title_width = title.len();

    // Total inner width is either title size or content size, whichever is larger
    let inner_width = min_content_width.max(title_width);

    // Recompute max_val_len so it spans the remaining space
    let value_column_width = inner_width.saturating_sub(max_key_len + 3);

    let mut out = String::new();

    // Top border
    let _ = writeln!(out, "╭{}╮", "─".repeat(inner_width + 2));

    // Title row
    let _ = writeln!(out, "│ {title:<inner_width$} │");

    // Separator
    let _ = writeln!(
        out,
        "├{}┬{}┤",
        "─".repeat(max_key_len + 2),
        "─".repeat(value_column_width + 2)
    );

    // Rows
    for (key, val) in rows {
        let _ = writeln!(out, "│ {key:<max_key_len$} │ {val:<value_column_width$} │");
    }

    // Bottom border
    let _ = write!(
        out,
        "╰{}┴{}╯",
        "─".repeat(max_key_len + 2),
        "─".repeat(value_column_width + 2)
    );

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn stats_generates_correct_output_for_sample_pattern() {
        let source = "pattern = fast(2, bd sn)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let stats = sample_pattern_stats("pattern", pattern, 2).unwrap();
        assert!(stats.contains("Pattern Stats: pattern (2 cycles)"));
        assert!(stats.contains("│ Total Events   │ 8                 │"));
        assert!(stats.contains("│ Unique Samples │ 2 (bd, sn)        │"));
        assert!(stats.contains("│ Event Density  │ 4.00 events/cycle │"));
    }

    #[test]
    fn stats_generates_correct_output_for_number_pattern() {
        let source = "pattern = 1 2 3";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let stats = number_pattern_stats("pattern", pattern, 1).unwrap();
        assert!(stats.contains("Pattern Stats: pattern (1 cycles)"));
        assert!(stats.contains("│ Total Events  │ 3                 │"));
        assert!(stats.contains("│ Min Value     │ 1.000             │"));
        assert!(stats.contains("│ Max Value     │ 3.000             │"));
        assert!(stats.contains("│ Average Value │ 2.000             │"));
        assert!(stats.contains("│ Event Density │ 3.00 events/cycle │"));
    }

    #[test]
    fn stats_returns_error_for_zero_cycles_sample() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let error = sample_pattern_stats("pattern", pattern, 0).unwrap_err();
        assert_eq!(error.to_string(), "stats requires at least one cycle");
    }

    #[test]
    fn stats_returns_error_for_zero_cycles_number() {
        let source = "pattern = 1 2 3";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let error = number_pattern_stats("pattern", pattern, 0).unwrap_err();
        assert_eq!(error.to_string(), "stats requires at least one cycle");
    }
}
