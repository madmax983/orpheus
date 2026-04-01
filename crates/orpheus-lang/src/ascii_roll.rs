//! The `ascii_roll` module renders patterns as ASCII-art piano rolls.
//!
//! This module is used by the REPL and TUI to visualize the scheduled
//! events of a pattern in the terminal, showing time on the x-axis.
use std::collections::BTreeMap;
use std::fmt::Write;

use crate::eval::{EvalError, render_span};
use crate::value::SamplePatternValue;

/// Renders a sample pattern's evaluated events to an ASCII piano roll string.
///
/// The roll visualizes the given number of `cycles` across a configurable
/// `steps_per_cycle` resolution.
///
/// # Panics
///
/// This function does not panic.
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if `cycle_count` is 0.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, render_ascii_roll, Value};
///
/// let source = "song = fast(2, bd sn)";
/// let bindings = eval_module(source, ReplMode::Loose).unwrap();
/// let Value::SamplePattern(pattern) = bindings.get("song").unwrap() else { panic!() };
///
/// // Render 1 cycle with 8 steps of resolution.
/// let roll = render_ascii_roll(pattern, 1, 8).unwrap();
///
/// assert!(roll.contains("bd"));
/// assert!(roll.contains("sn"));
/// ```
pub fn render_ascii_roll(
    pattern: &SamplePatternValue,
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

    let mut lanes: BTreeMap<String, Vec<char>> = BTreeMap::new();
    let total_steps = usize::try_from(cycle_count * u64::from(steps_per_cycle))?;

    // Determine unique samples to initialize lanes
    for event in &events {
        let sample = event.value.sample().to_string();
        lanes
            .entry(sample)
            .or_insert_with(|| vec!['.'; total_steps]);
    }

    // Populate the grid
    for event in events {
        let sample = event.value.sample().to_string();
        let lane = lanes.get_mut(&sample).unwrap_or_else(|| {
            unreachable!("lane was initialized in previous loop");
        });

        let start_f64 = f64::from(event.part.start());
        let end_f64 = f64::from(event.part.end());

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start_step = (start_f64 * f64::from(steps_per_cycle)).round() as usize;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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
            // Handle instantaneous triggers that might round to the same step
            lane[start_step] = 'x';
        }
    }

    let max_label_len = lanes
        .keys()
        .map(std::string::String::len)
        .max()
        .unwrap_or(0);
    let mut output = String::new();

    for (sample, grid) in lanes {
        write!(output, "{sample:>max_label_len$} | ")?;

        for (i, &c) in grid.iter().enumerate() {
            if i > 0 && i % (steps_per_cycle as usize) == 0 {
                output.push('|');
            }
            output.push(c);
        }
        output.push('\n');
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn ascii_roll_generates_correct_grid() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let roll = render_ascii_roll(pattern, 1, 8).unwrap();

        assert!(roll.contains("bd | x---...."));
        assert!(roll.contains("sn | ....x---"));
    }

    #[test]
    fn ascii_roll_handles_fast_pattern() {
        let source = "pattern = fast(2, bd sn)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let roll = render_ascii_roll(pattern, 1, 8).unwrap();

        assert!(roll.contains("bd | x-..x-.."));
        assert!(roll.contains("sn | ..x-..x-"));
    }
}
