//! The `stats` module provides analytical utilities for evaluated patterns.
//!
//! This module implements the core logic for the `:stats` REPL command, calculating
//! metadata such as total event count, unique samples, density, and numerical ranges
//! from a concrete pattern over a specified number of cycles.

use std::collections::BTreeSet;

use comfy_table::{Cell, Table, presets::UTF8_BORDERS_ONLY};
use crossterm::style::Stylize;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Analyzes a sample pattern's evaluated events and returns a formatted report.
///
/// The report contains the total number of events, unique samples triggered,
/// and the event density (events per cycle).
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, sample_pattern_stats};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let stats = sample_pattern_stats("x", pattern, 2).unwrap();
/// println!("{stats}");
/// ```
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
        samples.insert(event.value.sample());
    }

    let unique_count = samples.len();
    // ⚡ Bolt: By pre-allocating a single `String` buffer and iteratively pushing `&str` references,
    // we completely eliminate the intermediate `Vec` allocation required by `.collect::<Vec<_>>().join(", ")`.
    // We also avoid `.to_string()` clones for every unique sample by storing `&str` references in the `BTreeSet`.
    let mut sample_list = String::with_capacity(unique_count * 8);
    let mut first = true;
    for sample in samples {
        if !first {
            sample_list.push_str(", ");
        }
        sample_list.push_str(sample);
        first = false;
    }

    #[allow(clippy::cast_precision_loss)]
    let density = (total_events as f64) / (cycle_count as f64);

    let title = format!(
        "{} {binding_name} ({} cycles)",
        "Pattern Stats:".cyan().bold(),
        cycle_count.to_string().yellow()
    );
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);

    table.add_row(vec![
        Cell::new("Total Events").fg(comfy_table::Color::DarkGrey),
        Cell::new(total_events.to_string()).fg(comfy_table::Color::Green),
    ]);
    table.add_row(vec![
        Cell::new("Unique Samples").fg(comfy_table::Color::DarkGrey),
        Cell::new(format!("{unique_count} ({sample_list})")).fg(comfy_table::Color::Yellow),
    ]);
    table.add_row(vec![
        Cell::new("Event Density").fg(comfy_table::Color::DarkGrey),
        Cell::new(format!("{density:.2} events/cycle")).fg(comfy_table::Color::Cyan),
    ]);

    Ok(format!("{title}\n{table}"))
}

/// Analyzes a number pattern's evaluated events and returns a formatted report.
///
/// The report contains the total number of events, minimum value, maximum value,
/// average value, and the event density (events per cycle).
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, number_pattern_stats};
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let stats = number_pattern_stats("x", pattern, 2).unwrap();
/// println!("{stats}");
/// ```
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

    let title = format!(
        "{} {binding_name} ({} cycles)",
        "Pattern Stats:".cyan().bold(),
        cycle_count.to_string().yellow()
    );
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);

    table.add_row(vec![
        Cell::new("Total Events").fg(comfy_table::Color::DarkGrey),
        Cell::new(total_events.to_string()).fg(comfy_table::Color::Green),
    ]);
    table.add_row(vec![
        Cell::new("Min Value").fg(comfy_table::Color::DarkGrey),
        Cell::new(format!("{min_val:.3}")).fg(comfy_table::Color::Yellow),
    ]);
    table.add_row(vec![
        Cell::new("Max Value").fg(comfy_table::Color::DarkGrey),
        Cell::new(format!("{max_val:.3}")).fg(comfy_table::Color::Yellow),
    ]);
    table.add_row(vec![
        Cell::new("Average Value").fg(comfy_table::Color::DarkGrey),
        Cell::new(format!("{avg:.3}")).fg(comfy_table::Color::Yellow),
    ]);
    table.add_row(vec![
        Cell::new("Event Density").fg(comfy_table::Color::DarkGrey),
        Cell::new(format!("{density:.2} events/cycle")).fg(comfy_table::Color::Cyan),
    ]);

    Ok(format!("{title}\n{table}"))
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
        assert!(stats.contains("Pattern Stats:"));
        assert!(stats.contains("Total Events"));
        assert!(stats.contains('8'));
        assert!(stats.contains("Unique Samples"));
        assert!(stats.contains("2 (bd, sn)"));
        assert!(stats.contains("Event Density"));
        assert!(stats.contains("4.00 events/cycle"));
    }

    #[test]
    fn stats_generates_correct_output_for_number_pattern() {
        let source = "pattern = 1 2 3";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let stats = number_pattern_stats("pattern", pattern, 1).unwrap();
        assert!(stats.contains("Pattern Stats:"));
        assert!(stats.contains("Total Events"));
        assert!(stats.contains('3'));
        assert!(stats.contains("Min Value"));
        assert!(stats.contains("1.000"));
        assert!(stats.contains("Max Value"));
        assert!(stats.contains("3.000"));
        assert!(stats.contains("Average Value"));
        assert!(stats.contains("2.000"));
        assert!(stats.contains("Event Density"));
        assert!(stats.contains("3.00 events/cycle"));
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
