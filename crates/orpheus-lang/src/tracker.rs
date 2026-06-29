//! The `tracker` module provides visual rendering of patterns as text-based trackers.
//!
//! This module exports evaluated patterns into a `.trk` document, visualizing the
//! scheduled events in a vertical, columnar format inspired by classic tracker software.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

fn format_sample_name(sample: &str) -> String {
    if sample.len() > 4 {
        sample.chars().take(4).collect::<String>()
    } else {
        sample.to_string()
    }
}

/// Exports a sample pattern's evaluated events to a Tracker text file.
///
/// The tracker output displays time vertically (as rows corresponding to 1/16th cycle steps)
/// and distinct sample types horizontally (as columns).
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_sample_pattern_to_tracker};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.trk");
/// export_sample_pattern_to_tracker(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails, the cycle count is 0, or if the file cannot be written.
#[allow(clippy::missing_panics_doc, clippy::cast_precision_loss)]
pub fn export_sample_pattern_to_tracker(
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
        samples.insert(event.value.sample());
    }
    // ⚡ Bolt: Use a sorted vector of string slices instead of allocating Strings
    let sample_list: Vec<&str> = samples.into_iter().collect();

    // Resolution: 16 steps per cycle
    let steps_per_cycle = 16_u32;
    let total_steps = usize::try_from(cycle_count * u64::from(steps_per_cycle))?;

    if total_steps > 100_000 {
        return Err(EvalError::new(
            "evaluation exceeded the maximum allowed event limit",
        ));
    }

    // Create a grid of dimensions: [total_steps][sample_list.len()]
    // Each cell will optionally contain a formatted string of the sample name (if triggered)
    // or the delay/continuation character.
    let mut grid: Vec<Vec<Option<String>>> = vec![vec![None; sample_list.len()]; total_steps];

    for event in &events {
        let sample = event.value.sample().to_string();
        let lane_idx = sample_list
            .iter()
            .position(|s| *s == sample)
            .ok_or_else(|| {
                crate::EvalError::new(format!("sample '{sample}' not found in lane list"))
            })?;

        let start_f64 = f64::from(event.part.start());
        let end_f64 = f64::from(event.part.end());

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start_step = (start_f64 * f64::from(steps_per_cycle)).round() as usize;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let end_step = (end_f64 * f64::from(steps_per_cycle)).round() as usize;

        let start_step = start_step.min(total_steps);
        let end_step = end_step.min(total_steps);

        if start_step < end_step {
            grid[start_step][lane_idx] = Some(format_sample_name(&sample));
            for item in grid.iter_mut().take(end_step).skip(start_step + 1) {
                if item[lane_idx].is_none() {
                    item[lane_idx] = Some("====".to_string());
                }
            }
        } else if start_step < total_steps && grid[start_step][lane_idx].is_none() {
            grid[start_step][lane_idx] = Some(format_sample_name(&sample));
        }
    }

    let mut file = std::fs::File::create(path)?;
    writeln!(file, "Orpheus Tracker Export")?;
    writeln!(file, "Cycles: {cycle_count}, Resolution: 1/16")?;
    writeln!(file, "=========================================")?;

    // Print Header
    write!(file, " STEP | TIME  |")?;
    for sample in &sample_list {
        let padded = format_sample_name(sample);
        write!(file, " {padded:4} |")?;
    }
    writeln!(file)?;

    // Print Separator
    write!(file, "------+-------+")?;
    for _ in &sample_list {
        write!(file, "------+")?;
    }
    writeln!(file)?;

    // Print Grid Rows
    for (step, _val_opt) in grid.iter().enumerate().take(total_steps) {
        let cycle_num = step / (steps_per_cycle as usize);
        let sub_step = step % (steps_per_cycle as usize);
        let time = (step as f64) / f64::from(steps_per_cycle);

        write!(file, " {cycle_num:02}:{sub_step:02} | {time:4.2} |")?;

        for item in grid[step].iter().take(sample_list.len()) {
            if let Some(val) = item {
                write!(file, " {val:4} |")?;
            } else {
                write!(file, " ---- |")?;
            }
        }
        writeln!(file)?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a Tracker text file.
///
/// The tracker output displays time vertically and numeric values in a single column.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_tracker};
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_num.trk");
/// export_number_pattern_to_tracker(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails, the cycle count is 0, or if the file cannot be written.
#[allow(clippy::missing_panics_doc, clippy::cast_precision_loss)]
pub fn export_number_pattern_to_tracker(
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

    let steps_per_cycle = 16_u32;
    let total_steps = usize::try_from(cycle_count * u64::from(steps_per_cycle))?;

    if total_steps > 100_000 {
        return Err(EvalError::new(
            "evaluation exceeded the maximum allowed event limit",
        ));
    }

    let mut grid: Vec<Option<String>> = vec![None; total_steps];

    for event in &events {
        let start_f64 = f64::from(event.part.start());
        let end_f64 = f64::from(event.part.end());

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start_step = (start_f64 * f64::from(steps_per_cycle)).round() as usize;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let end_step = (end_f64 * f64::from(steps_per_cycle)).round() as usize;

        let start_step = start_step.min(total_steps);
        let end_step = end_step.min(total_steps);
        let val_str = format!("{:7.2}", event.value);

        if start_step < end_step {
            grid[start_step] = Some(val_str);
            for item in grid.iter_mut().take(end_step).skip(start_step + 1) {
                if item.is_none() {
                    *item = Some("=======".to_string());
                }
            }
        } else if start_step < total_steps && grid[start_step].is_none() {
            grid[start_step] = Some(val_str);
        }
    }

    let mut file = std::fs::File::create(path)?;
    writeln!(file, "Orpheus Tracker Export")?;
    writeln!(file, "Cycles: {cycle_count}, Resolution: 1/16")?;
    writeln!(file, "=========================================")?;

    // Print Header
    writeln!(file, " STEP | TIME  | VALUE   |")?;
    writeln!(file, "------+-------+---------+")?;

    // Print Grid Rows
    for (step, _val_opt) in grid.iter().enumerate().take(total_steps) {
        let cycle_num = step / (steps_per_cycle as usize);
        let sub_step = step % (steps_per_cycle as usize);
        let time = (step as f64) / f64::from(steps_per_cycle);

        write!(file, " {cycle_num:02}:{sub_step:02} | {time:4.2} |")?;

        if let Some(val) = &grid[step] {
            write!(file, " {val} |")?;
        } else {
            write!(file, " ------- |")?;
        }
        writeln!(file)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn temp_trk_path() -> PathBuf {
        std::env::temp_dir().join(format!("orpheus-export-{}.trk", unique_temp_suffix()))
    }

    fn unique_temp_suffix() -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        format!("{timestamp}-{counter}")
    }

    #[test]
    fn tracker_exporter_generates_sample_pattern() {
        let source = "pattern = fast(2, bd sn)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path = temp_trk_path();
        export_sample_pattern_to_tracker(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Orpheus Tracker Export"));
        assert!(content.contains("bd"));
        assert!(content.contains("sn"));
        assert!(content.contains("00:00 | 0.00 |"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn tracker_exporter_generates_number_pattern() {
        let source = "pattern = 1 2 3";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path = temp_trk_path();
        export_number_pattern_to_tracker(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("Orpheus Tracker Export"));
        assert!(content.contains("VALUE"));
        assert!(content.contains("1.00"));
        assert!(content.contains("3.00"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();
        assert_eq!(
            export_sample_pattern_to_tracker(pat, "test.trk", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();
        assert_eq!(
            export_number_pattern_to_tracker(pat, "test.trk", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
