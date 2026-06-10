//! The `blender_export` module provides an exporter to Python scripts for Blender.
//!
//! This exporter generates a `.py` script that uses the `bpy` module
//! to construct 3D representations of Orpheus patterns, like bouncing cubes
//! for sample events or moving spheres for number events.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to a Python script for Blender.
///
/// Generates a `.py` script that creates bouncing cubes for each sample triggered.
/// The script handles importing `bpy` and creating/animating the cubes.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_sample_pattern_to_blender};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("blender_drums.py");
/// export_sample_pattern_to_blender(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn export_sample_pattern_to_blender(
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
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "import bpy")?;
    writeln!(file, "import math")?;
    writeln!(file)?;
    writeln!(file, "bpy.context.scene.frame_start = 0")?;

    // Assume 120 BPM -> 1 cycle = 2 seconds, 24 fps -> 48 frames per cycle
    let fps = 24.0;
    let seconds_per_cycle = 2.0;
    let frames_per_cycle = fps * seconds_per_cycle;

    writeln!(
        file,
        "bpy.context.scene.frame_end = {}",
        (frames_per_cycle * (cycle_count as f64)) as u32
    )?;
    writeln!(file)?;
    writeln!(file, "# Clear existing meshes")?;
    writeln!(file, "bpy.ops.object.select_all(action='DESELECT')")?;
    writeln!(file, "bpy.ops.object.select_by_type(type='MESH')")?;
    writeln!(file, "bpy.ops.object.delete()")?;
    writeln!(file)?;

    let mut samples: Vec<String> = events
        .iter()
        .map(|e| e.value.sample().to_string())
        .collect();
    samples.sort();
    samples.dedup();

    for (idx, sample) in samples.iter().enumerate() {
        writeln!(
            file,
            "bpy.ops.mesh.primitive_cube_add(size=1.0, location=({}, 0, 0))",
            idx as f64 * 2.0
        )?;
        writeln!(file, "obj_{sample} = bpy.context.active_object")?;
        writeln!(file, "obj_{sample}.name = '{sample}'")?;
    }

    for event in &events {
        let sample = event.value.sample();
        let start_frame = (f64::from(event.part.start()) * frames_per_cycle).round() as u32;
        let gain = event.value.gain();
        let height = gain * 2.0;

        let frame_minus_2 = start_frame.saturating_sub(2);
        let frame_plus_2 = start_frame + 2;

        writeln!(file)?;
        writeln!(file, "obj_{sample}.location.z = 0")?;
        writeln!(
            file,
            "obj_{sample}.keyframe_insert(data_path='location', frame={frame_minus_2})"
        )?;
        writeln!(file, "obj_{sample}.location.z = {height}")?;
        writeln!(
            file,
            "obj_{sample}.keyframe_insert(data_path='location', frame={start_frame})"
        )?;
        writeln!(file, "obj_{sample}.location.z = 0")?;
        writeln!(
            file,
            "obj_{sample}.keyframe_insert(data_path='location', frame={frame_plus_2})"
        )?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a Python script for Blender.
///
/// Generates a `.py` script that animates a sphere moving across X (time) and Z (value).
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_blender};
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("blender_automation.py");
/// export_number_pattern_to_blender(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn export_number_pattern_to_blender(
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
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "import bpy")?;
    writeln!(file)?;
    writeln!(file, "bpy.context.scene.frame_start = 0")?;

    let fps = 24.0;
    let seconds_per_cycle = 2.0;
    let frames_per_cycle = fps * seconds_per_cycle;

    writeln!(
        file,
        "bpy.context.scene.frame_end = {}",
        (frames_per_cycle * (cycle_count as f64)) as u32
    )?;
    writeln!(file)?;
    writeln!(file, "# Clear existing meshes")?;
    writeln!(file, "bpy.ops.object.select_all(action='DESELECT')")?;
    writeln!(file, "bpy.ops.object.select_by_type(type='MESH')")?;
    writeln!(file, "bpy.ops.object.delete()")?;
    writeln!(file)?;

    writeln!(
        file,
        "bpy.ops.mesh.primitive_uv_sphere_add(radius=0.5, location=(0, 0, 0))"
    )?;
    writeln!(file, "obj_sphere = bpy.context.active_object")?;
    writeln!(file, "obj_sphere.name = 'ValueSphere'")?;

    for event in &events {
        let start_frame = (f64::from(event.part.start()) * frames_per_cycle).round() as u32;
        let value = event.value;

        // Map time to X and value to Z
        let loc_x = f64::from(event.part.start()) * 10.0;
        writeln!(file, "obj_sphere.location.x = {loc_x}")?;
        writeln!(file, "obj_sphere.location.z = {value}")?;
        writeln!(
            file,
            "obj_sphere.keyframe_insert(data_path='location', frame={start_frame})"
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn blender_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_output.py");
        export_sample_pattern_to_blender(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("import bpy"));
        assert!(content.contains("obj_bd = bpy.context.active_object"));
        assert!(content.contains("obj_bd.keyframe_insert"));
        assert!(content.contains("obj_sn = bpy.context.active_object"));
        assert!(content.contains("obj_sn.keyframe_insert"));
    }

    #[test]
    fn blender_exporter_generates_number_pattern() {
        let source = "pattern = 1 2";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_number_output.py");
        export_number_pattern_to_blender(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("import bpy"));
        assert!(content.contains("obj_sphere.name = 'ValueSphere'"));
        assert!(content.contains("obj_sphere.location.z = 1"));
        assert!(content.contains("obj_sphere.location.z = 2"));
        assert!(content.contains("obj_sphere.keyframe_insert"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_blender(pat, "test.py", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_blender(pat, "test.py", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
