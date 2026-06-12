//! The `blender_export` module provides an exporter to Blender Python scripts (`.py`).
//!
//! This exporter generates a `.py` script that can be run inside Blender to create
//! keyframed animations based on Orpheus patterns. For number patterns, it keyframes
//! object location or scale based on pitch. For sample patterns, it can trigger
//! object visibility or scale based on samples.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
const SECONDS_PER_CYCLE: f64 = 2.0;
const FRAMES_PER_SECOND: f64 = 24.0;
const FRAMES_PER_CYCLE: f64 = SECONDS_PER_CYCLE * FRAMES_PER_SECOND;

/// Exports a sample pattern's evaluated events to a Blender Python script.
///
/// Each event creates keyframes for scale or visibility.
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
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
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "import bpy")?;
    writeln!(file)?;
    writeln!(file, "# Orpheus Blender Sample Export")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(file)?;
    writeln!(file, "def create_or_get_object(name):")?;
    writeln!(file, "    if name in bpy.data.objects:")?;
    writeln!(file, "        return bpy.data.objects[name]")?;
    writeln!(file, "    bpy.ops.mesh.primitive_cube_add(size=1)")?;
    writeln!(file, "    obj = bpy.context.active_object")?;
    writeln!(file, "    obj.name = name")?;
    writeln!(file, "    return obj")?;
    writeln!(file)?;
    writeln!(file, "bpy.context.scene.frame_start = 1")?;
    #[allow(clippy::cast_precision_loss)]
    let end_frame = ((cycle_count as f64) * FRAMES_PER_CYCLE).round() as i32;
    writeln!(file, "bpy.context.scene.frame_end = {}", end_frame.max(1))?;
    writeln!(file, "bpy.context.scene.render.fps = 24")?;
    writeln!(file)?;

    for event in events {
        let start_frame = (f64::from(event.part.start()) * FRAMES_PER_CYCLE).round() as i32;
        let end_frame = (f64::from(event.part.end()) * FRAMES_PER_CYCLE).round() as i32;
        let start_frame = start_frame.max(1);
        let end_frame = end_frame.max(start_frame + 1);

        let sample = event.value.sample();
        let gain = event.value.gain();

        writeln!(file, "obj = create_or_get_object('{}')", sample)?;

        // Keyframe scale
        writeln!(file, "obj.scale = (1.0, 1.0, 1.0)")?;
        writeln!(
            file,
            "obj.keyframe_insert(data_path='scale', frame={})",
            (start_frame - 1).max(1)
        )?;
        writeln!(file, "obj.scale = ({0:.3}, {0:.3}, {0:.3})", 1.0 + gain)?;
        writeln!(
            file,
            "obj.keyframe_insert(data_path='scale', frame={})",
            start_frame
        )?;
        writeln!(file, "obj.scale = (1.0, 1.0, 1.0)")?;
        writeln!(
            file,
            "obj.keyframe_insert(data_path='scale', frame={})",
            end_frame
        )?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a Blender Python script.
///
/// Number patterns are assumed to represent pitch/value, creating keyframes
/// for location along the Z axis.
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
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
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "import bpy")?;
    writeln!(file)?;
    writeln!(file, "# Orpheus Blender Number Export")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(file)?;
    writeln!(file, "def create_or_get_object(name):")?;
    writeln!(file, "    if name in bpy.data.objects:")?;
    writeln!(file, "        return bpy.data.objects[name]")?;
    writeln!(file, "    bpy.ops.mesh.primitive_monkey_add(size=1)")?;
    writeln!(file, "    obj = bpy.context.active_object")?;
    writeln!(file, "    obj.name = name")?;
    writeln!(file, "    return obj")?;
    writeln!(file)?;
    writeln!(file, "bpy.context.scene.frame_start = 1")?;
    #[allow(clippy::cast_precision_loss)]
    let end_frame = ((cycle_count as f64) * FRAMES_PER_CYCLE).round() as i32;
    writeln!(file, "bpy.context.scene.frame_end = {}", end_frame.max(1))?;
    writeln!(file, "bpy.context.scene.render.fps = 24")?;
    writeln!(file)?;

    writeln!(file, "obj = create_or_get_object('MelodyTarget')")?;

    for event in events {
        let start_frame = (f64::from(event.part.start()) * FRAMES_PER_CYCLE).round() as i32;
        let start_frame = start_frame.max(1);

        let pitch = event.value;

        // Keyframe Z location
        writeln!(file, "obj.location[2] = {0:.3}", pitch / 10.0)?;
        writeln!(
            file,
            "obj.keyframe_insert(data_path='location', index=2, frame={})",
            start_frame
        )?;
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
    fn blender_exporter_generates_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join(format!(
            "test_blender_sample_output_{}.py",
            unique_temp_suffix()
        ));
        export_sample_pattern_to_blender(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("import bpy"));
        assert!(content.contains("obj = create_or_get_object('bd')"));
        assert!(content.contains("obj.keyframe_insert(data_path='scale'"));
    }

    #[test]
    fn blender_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join(format!(
            "test_blender_number_output_{}.py",
            unique_temp_suffix()
        ));
        export_number_pattern_to_blender(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("import bpy"));
        assert!(content.contains("obj = create_or_get_object('MelodyTarget')"));
        assert!(content.contains("obj.location[2] = 6.000"));
        assert!(content.contains("obj.keyframe_insert(data_path='location'"));
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
