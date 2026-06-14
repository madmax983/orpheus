//! The `blender_export` module provides an exporter to Blender Python Scripts.
//!
//! This exporter translates evaluated number patterns into a Python script
//! that can be executed inside Blender to automatically animate object
//! locations (e.g., the Z-axis) over time, allowing synchronized 3D graphics.

use std::io::Write;
use std::path::Path;

use crate::eval::render_span;
use crate::error::EvalError;
use crate::value::NumberPatternValue;

// Assuming 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
const SECONDS_PER_CYCLE: f64 = 2.0;

// Blender default is usually 24 FPS
const FRAMES_PER_SECOND: f64 = 24.0;

/// Exports a number pattern's evaluated events to a Blender Python script.
///
/// Number patterns are mapped to the Z-axis location of a target object in Blender.
/// The script, when executed in Blender's Text Editor, will automatically insert
/// keyframes mapping the temporal structure of the Orpheus pattern onto the timeline.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_blender;
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("animation.py");
/// export_number_pattern_to_blender(pattern, &path, 2, "Cube", 2.0).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_blender(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
    object_name: &str,
    scale: f64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "import bpy")?;
    writeln!(file, "import math")?;
    writeln!(file)?;
    writeln!(file, "# Orpheus Blender Animation Export")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(file)?;
    writeln!(file, "target_obj_name = \"{object_name}\"")?;
    writeln!(file, "if target_obj_name in bpy.data.objects:")?;
    writeln!(file, "    obj = bpy.data.objects[target_obj_name]")?;
    writeln!(file, "    obj.animation_data_clear()")?;
    writeln!(file, "else:")?;
    writeln!(
        file,
        "    bpy.ops.mesh.primitive_cube_add(size=1.0, location=(0,0,0))"
    )?;
    writeln!(file, "    obj = bpy.context.active_object")?;
    writeln!(file, "    obj.name = target_obj_name")?;
    writeln!(file)?;
    writeln!(
        file,
        "bpy.context.scene.render.fps = {}",
        FRAMES_PER_SECOND as u32
    )?;
    writeln!(file)?;

    for event in events {
        // Compute frame indices
        let start_time_sec = event.part.start().to_f64() * SECONDS_PER_CYCLE;
        let start_frame = (start_time_sec * FRAMES_PER_SECOND).round() as u64;

        let end_time_sec = event.part.end().to_f64() * SECONDS_PER_CYCLE;
        let end_frame = (end_time_sec * FRAMES_PER_SECOND).round() as u64;

        let val = event.value * scale;

        // Insert keyframes for a pulse: neutral -> jump -> neutral

        // Before jump
        let pre_frame = if start_frame > 0 { start_frame - 1 } else { 0 };
        writeln!(file, "obj.location[2] = 0.0")?;
        writeln!(
            file,
            "obj.keyframe_insert(data_path=\"location\", index=2, frame={pre_frame})"
        )?;

        // At start (jump)
        writeln!(file, "obj.location[2] = {val}")?;
        writeln!(
            file,
            "obj.keyframe_insert(data_path=\"location\", index=2, frame={start_frame})"
        )?;

        // Hold to end
        writeln!(
            file,
            "obj.keyframe_insert(data_path=\"location\", index=2, frame={end_frame})"
        )?;

        // Drop back to neutral
        let post_frame = end_frame + 1;
        writeln!(file, "obj.location[2] = 0.0")?;
        writeln!(
            file,
            "obj.keyframe_insert(data_path=\"location\", index=2, frame={post_frame})"
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};
    use std::fs;

    #[test]
    fn test_blender_export() {
        let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("blender_test.py");

        export_number_pattern_to_blender(pattern, &path, 1, "MyCube", 1.5).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("import bpy"));
        assert!(content.contains("target_obj_name = \"MyCube\""));
        assert!(content.contains("bpy.context.scene.render.fps = 24"));
        assert!(content.contains("obj.location[2] = 1.5"));
        assert!(content.contains("obj.location[2] = 3"));
        assert!(content.contains("obj.location[2] = 4.5"));
    }

    #[test]
    fn test_blender_export_zero_cycles() {
        let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
        let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
        let path = std::env::temp_dir().join("blender_test_zero.py");

        let res = export_number_pattern_to_blender(pattern, &path, 0, "Cube", 1.0);
        assert!(res.is_err());
    }
}
