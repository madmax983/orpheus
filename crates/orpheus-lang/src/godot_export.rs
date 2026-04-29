//! The `godot_export` module provides an exporter to Godot 4 scene files.
//!
//! This exporter generates a `.tscn` scene containing `CSGBox3D` nodes
//! to visualize evaluated Orpheus patterns in 3D space.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to a Godot 4 `.tscn` file.
///
/// Each event is represented as a `CSGBox3D`. Time maps to the X axis,
/// and different samples map to the Z axis.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::godot_export::export_sample_pattern_to_godot;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("scene.tscn");
/// export_sample_pattern_to_godot(pattern, &path, 2).unwrap();
/// ```
///
/// # Panics
///
/// Panics if a sample's name cannot be found in the previously collected set of sample names.
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn export_sample_pattern_to_godot(
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

    writeln!(file, "[gd_scene format=3]")?;
    writeln!(file)?;
    writeln!(file, "[node name=\"OrpheusPattern\" type=\"Node3D\"]")?;

    let seconds_per_cycle = 2.0;

    let mut samples = std::collections::BTreeSet::new();
    for event in &events {
        samples.insert(event.value.sample());
    }
    let sample_list: Vec<&str> = samples.into_iter().collect();

    for (i, event) in events.iter().enumerate() {
        let sample = event.value.sample();
        let lane_idx = sample_list.iter().position(|s| *s == sample).unwrap();

        let start = f64::from(event.part.start()) * seconds_per_cycle;
        let end = f64::from(event.part.end()) * seconds_per_cycle;
        let duration = end - start;

        // Position: X is time center, Y is 0, Z is lane
        let x = start + (duration / 2.0);
        let y = 0.0;
        let z = (lane_idx as f64) * 2.0;

        let width = duration * 0.9; // Slight gap between notes
        let height = 0.5;
        let depth = 1.0;

        writeln!(file)?;
        writeln!(
            file,
            "[node name=\"Event_{i}_{sample}\" type=\"CSGBox3D\" parent=\".\"]"
        )?;
        writeln!(
            file,
            "transform = Transform3D(1, 0, 0, 0, 1, 0, 0, 0, 1, {x:.3}, {y:.3}, {z:.3})"
        )?;
        writeln!(file, "size = Vector3({width:.3}, {height:.3}, {depth:.3})")?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a Godot 4 `.tscn` file.
///
/// Each event is represented as a `CSGBox3D`. Time maps to the X axis,
/// and the value (pitch) maps to the Y axis.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::godot_export::export_number_pattern_to_godot;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("scene_num.tscn");
/// export_number_pattern_to_godot(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_godot(
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

    writeln!(file, "[gd_scene format=3]")?;
    writeln!(file)?;
    writeln!(file, "[node name=\"OrpheusPattern\" type=\"Node3D\"]")?;

    let seconds_per_cycle = 2.0;

    for (i, event) in events.iter().enumerate() {
        let start = f64::from(event.part.start()) * seconds_per_cycle;
        let end = f64::from(event.part.end()) * seconds_per_cycle;
        let duration = end - start;

        // Position: X is time center, Y is pitch, Z is 0
        let x = start + (duration / 2.0);
        let y = event.value;
        let z = 0.0;

        let width = duration * 0.9;
        let height = 0.5;
        let depth = 1.0;

        writeln!(file)?;
        writeln!(
            file,
            "[node name=\"Event_{i}\" type=\"CSGBox3D\" parent=\".\"]"
        )?;
        writeln!(
            file,
            "transform = Transform3D(1, 0, 0, 0, 1, 0, 0, 0, 1, {x:.3}, {y:.3}, {z:.3})"
        )?;
        writeln!(file, "size = Vector3({width:.3}, {height:.3}, {depth:.3})")?;
    }

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
    fn godot_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_sample_output_{}.tscn", unique_temp_suffix()));
        export_sample_pattern_to_godot(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("[gd_scene format=3]"));
        assert!(content.contains("[node name=\"OrpheusPattern\" type=\"Node3D\"]"));
        assert!(content.contains("[node name=\"Event_0_bd\" type=\"CSGBox3D\" parent=\".\"]"));
        assert!(content.contains("[node name=\"Event_1_sn\" type=\"CSGBox3D\" parent=\".\"]"));
    }

    #[test]
    fn godot_exporter_generates_number_pattern() {
        let source = "pattern = 60 62";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_number_output_{}.tscn", unique_temp_suffix()));
        export_number_pattern_to_godot(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("[gd_scene format=3]"));
        assert!(content.contains("[node name=\"OrpheusPattern\" type=\"Node3D\"]"));
        assert!(content.contains("[node name=\"Event_0\" type=\"CSGBox3D\" parent=\".\"]"));
        assert!(content.contains("60.000"));
        assert!(content.contains("[node name=\"Event_1\" type=\"CSGBox3D\" parent=\".\"]"));
        assert!(content.contains("62.000"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_godot(pat, "test.tscn", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_godot(pat, "test.tscn", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
