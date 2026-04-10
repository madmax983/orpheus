//! The `obj` module provides utilities for exporting patterns to Wavefront OBJ 3D models.
//!
//! This enables 3D visualization of Orpheus musical patterns, converting time (cycles),
//! discrete instruments/values, and velocities into spatial geometry (X, Y, Z axes).

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to a Wavefront OBJ file.
///
/// Each sample event is rendered as a 3D box. Time maps to the X axis,
/// sample lane to the Y axis, and gain to the Z axis (height).
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(clippy::cast_precision_loss)]
pub fn export_sample_pattern_to_obj(
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

    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(file, "# Orpheus Sample Pattern 3D Export")
        .map_err(|e| EvalError::new(e.to_string()))?;

    let mut vertex_count = 1;

    for event in &events {
        let sample = event.value.sample().to_string();
        let lane_idx = sample_list.iter().position(|s| *s == sample).unwrap();

        let start = f64::from(event.part.start());
        let end = f64::from(event.part.end());

        // Scale factors for visual proportions
        let x_start = start * 10.0;
        let x_end = end * 10.0;
        let y_pos = lane_idx as f64 * 2.0;
        let z_height = event.value.gain() * 2.0;
        let y_width = 1.0;

        // 8 vertices for a box
        // Bottom face (z=0)
        writeln!(file, "v {:.4} {:.4} 0.0000", x_start, y_pos)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "v {:.4} {:.4} 0.0000", x_end, y_pos)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "v {:.4} {:.4} 0.0000", x_end, y_pos + y_width)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "v {:.4} {:.4} 0.0000", x_start, y_pos + y_width)
            .map_err(|e| EvalError::new(e.to_string()))?;
        // Top face (z=height)
        writeln!(file, "v {:.4} {:.4} {:.4}", x_start, y_pos, z_height)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "v {:.4} {:.4} {:.4}", x_end, y_pos, z_height)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(
            file,
            "v {:.4} {:.4} {:.4}",
            x_end,
            y_pos + y_width,
            z_height
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(
            file,
            "v {:.4} {:.4} {:.4}",
            x_start,
            y_pos + y_width,
            z_height
        )
        .map_err(|e| EvalError::new(e.to_string()))?;

        let v = vertex_count;
        writeln!(file, "f {} {} {} {}", v, v + 1, v + 2, v + 3)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "f {} {} {} {}", v + 4, v + 5, v + 6, v + 7)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "f {} {} {} {}", v, v + 1, v + 5, v + 4)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "f {} {} {} {}", v + 3, v + 2, v + 6, v + 7)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "f {} {} {} {}", v, v + 3, v + 7, v + 4)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "f {} {} {} {}", v + 1, v + 2, v + 6, v + 5)
            .map_err(|e| EvalError::new(e.to_string()))?;

        vertex_count += 8;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to a Wavefront OBJ file.
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_obj(
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
    for event in &events {
        if event.value < min_val {
            min_val = event.value;
        }
    }
    if min_val.is_infinite() {
        min_val = 0.0;
    }

    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(file, "# Orpheus Number Pattern 3D Export")
        .map_err(|e| EvalError::new(e.to_string()))?;

    let mut vertex_count = 1;

    for event in &events {
        let start = f64::from(event.part.start());
        let end = f64::from(event.part.end());

        let x_start = start * 10.0;
        let x_end = end * 10.0;
        let y_pos = (event.value - min_val) * 2.0;
        let z_height = 1.0;
        let y_width = 1.0;

        // Bottom face (z=0)
        writeln!(file, "v {:.4} {:.4} 0.0000", x_start, y_pos)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "v {:.4} {:.4} 0.0000", x_end, y_pos)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "v {:.4} {:.4} 0.0000", x_end, y_pos + y_width)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "v {:.4} {:.4} 0.0000", x_start, y_pos + y_width)
            .map_err(|e| EvalError::new(e.to_string()))?;
        // Top face (z=height)
        writeln!(file, "v {:.4} {:.4} {:.4}", x_start, y_pos, z_height)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "v {:.4} {:.4} {:.4}", x_end, y_pos, z_height)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(
            file,
            "v {:.4} {:.4} {:.4}",
            x_end,
            y_pos + y_width,
            z_height
        )
        .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(
            file,
            "v {:.4} {:.4} {:.4}",
            x_start,
            y_pos + y_width,
            z_height
        )
        .map_err(|e| EvalError::new(e.to_string()))?;

        let v = vertex_count;
        writeln!(file, "f {} {} {} {}", v, v + 1, v + 2, v + 3)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "f {} {} {} {}", v + 4, v + 5, v + 6, v + 7)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "f {} {} {} {}", v, v + 1, v + 5, v + 4)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "f {} {} {} {}", v + 3, v + 2, v + 6, v + 7)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "f {} {} {} {}", v, v + 3, v + 7, v + 4)
            .map_err(|e| EvalError::new(e.to_string()))?;
        writeln!(file, "f {} {} {} {}", v + 1, v + 2, v + 6, v + 5)
            .map_err(|e| EvalError::new(e.to_string()))?;

        vertex_count += 8;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn obj_exporter_generates_sample_pattern() {
        let source = "pattern = fast(2, bd sn)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_sample_output.obj");
        export_sample_pattern_to_obj(pattern, &path, 2).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Orpheus Sample Pattern 3D Export"));
        assert!(content.contains("v 0.0000")); // Contains vertices
        assert!(content.contains("f 1 2 3 4")); // Contains faces
    }

    #[test]
    fn obj_exporter_generates_number_pattern() {
        let source = "pattern = 1 2 3";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_number_output.obj");
        export_number_pattern_to_obj(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("# Orpheus Number Pattern 3D Export"));
        assert!(content.contains("v 0.0000"));
        assert!(content.contains("f 1 2 3 4"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let err = export_sample_pattern_to_obj(pattern, "test.obj", 0).unwrap_err();
        assert_eq!(err.to_string(), "exporting requires at least one cycle");

        let source2 = "pattern2 = 1 2";
        let module2 = eval_module(source2, ReplMode::Loose).unwrap();
        let pattern2 = module2
            .get("pattern2")
            .unwrap()
            .as_number_pattern()
            .unwrap();

        let err2 = export_number_pattern_to_obj(pattern2, "test.obj", 0).unwrap_err();
        assert_eq!(err2.to_string(), "exporting requires at least one cycle");
    }
}
