//! The `number_roll` module renders numeric patterns as ASCII-art graphs.
//!
//! This module visualizes number patterns (such as pitches or control automation)
//! in the terminal. The Y-axis represents the values and the X-axis represents time.

use std::collections::{BTreeMap, BTreeSet};

use comfy_table::{Cell, CellAlignment, Table, presets::UTF8_BORDERS_ONLY};

use crossterm::style::Stylize;

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

/// Renders a number pattern's evaluated events to an ASCII plot string.
///
/// The roll visualizes the given number of `cycles` across a configurable
/// `steps_per_cycle` resolution. The Y-axis lists unique values in descending order.
///
/// # Panics
///
/// This function does not panic.
///
///
/// # Examples
///
/// ```
/// use orpheus_lang::{eval_module, ReplMode, render_ascii_number_roll};
///
/// let env = eval_module("x = 1 2", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let roll = render_ascii_number_roll("x", pattern, 1, 4).unwrap();
/// assert!(roll.contains("1.0"));
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if `cycle_count` is 0.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn render_ascii_number_roll(
    binding_name: &str,
    pattern: &NumberPatternValue,
    cycle_count: u64,
    steps_per_cycle: u32,
) -> Result<String, EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("rendering requires at least one cycle"));
    }
    if steps_per_cycle == 0 {
        return Err(EvalError::new("steps_per_cycle must be greater than zero"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;

    // We use integer formatting for the y-axis (multiplying by 1000 and rounding)
    // to group close values to the same lane, but we store the original formatted string.
    let mut unique_values: BTreeSet<i64> = BTreeSet::new();
    let mut value_strings: BTreeMap<i64, String> = BTreeMap::new();

    let total_steps = usize::try_from(cycle_count * u64::from(steps_per_cycle))?;

    if total_steps > 100_000 {
        return Err(EvalError::new(
            "evaluation exceeded the maximum allowed event limit",
        ));
    }

    // Determine unique values
    for event in &events {
        let val = event.value;
        let scaled_val = (val * 1000.0).round() as i64;
        unique_values.insert(scaled_val);
        value_strings
            .entry(scaled_val)
            .or_insert_with(|| format!("{val:.2}"));
    }

    // Sort lanes descending (highest values at the top)
    let sorted_values: Vec<i64> = unique_values.into_iter().rev().collect();
    let mut lanes: BTreeMap<i64, Vec<char>> = BTreeMap::new();
    for &val in &sorted_values {
        lanes.insert(val, vec!['.'; total_steps]);
    }

    // Populate the grid
    for event in events {
        let val = event.value;
        let scaled_val = (val * 1000.0).round() as i64;
        let lane = lanes.get_mut(&scaled_val).unwrap_or_else(|| {
            unreachable!("lane was initialized in previous loop");
        });

        let start_f64 = f64::from(event.part.start());
        let end_f64 = f64::from(event.part.end());

        let start_step = (start_f64 * f64::from(steps_per_cycle)).round() as usize;
        let end_step = (end_f64 * f64::from(steps_per_cycle)).round() as usize;

        let start_step = start_step.min(total_steps);
        let end_step = end_step.min(total_steps);

        if start_step < end_step {
            lane[start_step] = 'x';
            for item in lane.iter_mut().take(end_step).skip(start_step + 1) {
                if *item == '.' {
                    *item = '-';
                }
            }
        } else if start_step < total_steps && lane[start_step] == '.' {
            // Handle instantaneous triggers
            lane[start_step] = 'x';
        }
    }

    let title = format!(
        "{} {binding_name} ({} cycles)",
        "Number Roll:".cyan().bold(),
        cycle_count.to_string().yellow()
    );
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);

    for &val in &sorted_values {
        let grid = lanes.get(&val).unwrap();
        let label = value_strings.get(&val).unwrap();
        let mut row_string = String::new();
        for (i, &c) in grid.iter().enumerate() {
            if i > 0 && i % (steps_per_cycle as usize) == 0 {
                row_string.push('│');
            }
            row_string.push(c);
        }
        table.add_row(vec![
            Cell::new(label)
                .fg(comfy_table::Color::Cyan)
                .set_alignment(CellAlignment::Right),
            Cell::new(row_string).fg(comfy_table::Color::Green),
        ]);
    }

    Ok(format!("{title}\n{table}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn ascii_number_roll_generates_correct_grid() {
        let source = "pattern = 1 2";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let roll = render_ascii_number_roll("pattern", pattern, 1, 8).unwrap();

        assert!(roll.contains("Number Roll:"));
        assert!(roll.contains("1.00"));
        assert!(roll.contains("2.00"));
        // 2 is higher, so it's on top.
        // It occupies the second half: start at step 4
        // 1 occupies the first half: start at step 0
        assert!(roll.contains("....x---")); // 2.00
        assert!(roll.contains("x---....")); // 1.00
    }

    #[test]
    fn ascii_number_roll_handles_fast_pattern() {
        let source = "pattern = fast(2, 4 8)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let roll = render_ascii_number_roll("pattern", pattern, 1, 8).unwrap();

        assert!(roll.contains("Number Roll:"));
        assert!(roll.contains("4.00"));
        assert!(roll.contains("8.00"));
        assert!(roll.contains("x-..x-.."));
        assert!(roll.contains("..x-..x-"));
    }

    #[test]
    fn render_number_roll_zero_cycles() {
        let source = "pattern = 1 2 3";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let err = super::render_ascii_number_roll("pattern", pattern, 0, 8).unwrap_err();
        assert_eq!(err.to_string(), "rendering requires at least one cycle");
    }
}
