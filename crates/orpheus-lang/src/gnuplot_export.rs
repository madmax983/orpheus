//! The `gnuplot_export` module provides an exporter to Gnuplot scripts.
//!
//! This exporter generates a `.plt` script that visualizes a pattern's
//! events over time when run through `gnuplot`.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a number pattern's evaluated events to a Gnuplot script.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_gnuplot};
///
/// let env = eval_module("melody = 0 2 4 7", ReplMode::Loose).unwrap();
/// let pattern = env.get("melody").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("plot.plt");
/// export_number_pattern_to_gnuplot(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_gnuplot(
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

    let mut file = std::fs::File::create(path.as_ref())
        .map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "# Orpheus Gnuplot Export")?;
    writeln!(file, "set title 'Orpheus Number Pattern'")?;
    writeln!(file, "set xlabel 'Time (Cycles)'")?;
    writeln!(file, "set ylabel 'Value'")?;
    writeln!(file, "set grid")?;
    // We break the plot command into two writes to avoid formatting errors
    write!(file, "plot '-' using 1:2 with impulses lw 2 title 'Events', \\")?;
    writeln!(file)?;
    writeln!(file, "     '-' using 1:2 with points pt 7 ps 1.5 title ''")?;

    // Output data for impulses
    for event in &events {
        let start = f64::from(event.part.start());
        let val = event.value;
        writeln!(file, "{start:.3} {val:.3}")?;
    }
    writeln!(file, "e")?;

    // Output data for points
    for event in &events {
        let start = f64::from(event.part.start());
        let val = event.value;
        writeln!(file, "{start:.3} {val:.3}")?;
    }
    writeln!(file, "e")?;

    Ok(())
}

/// Exports a sample pattern's evaluated events to a Gnuplot script.
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
///
/// # Panics
///
/// Panics if a sample's name cannot be found in the previously collected set of sample names.
pub fn export_sample_pattern_to_gnuplot(
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

    let mut file = std::fs::File::create(path.as_ref())
        .map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "# Orpheus Gnuplot Export")?;
    writeln!(file, "set title 'Orpheus Sample Pattern'")?;
    writeln!(file, "set xlabel 'Time (Cycles)'")?;
    writeln!(file, "set ylabel 'Sample Index'")?;
    writeln!(file, "set grid")?;

    // Create a mapping from sample name to a Y-index for visualization
    let mut sample_names: Vec<String> = events.iter().map(|e| e.value.sample().to_string()).collect();
    sample_names.sort();
    sample_names.dedup();

    // Set y-ticks to the sample names
    let mut yticks = Vec::new();
    for (i, name) in sample_names.iter().enumerate() {
        yticks.push(format!("'{}' {}", name, i + 1));
    }
    if !yticks.is_empty() {
        writeln!(file, "set ytics ({})", yticks.join(", "))?;
    }

    writeln!(file, "set yrange [0:{}]", sample_names.len() + 1)?;

    write!(file, "plot '-' using 1:2 with impulses lw 2 title 'Events', \\")?;
    writeln!(file)?;
    writeln!(file, "     '-' using 1:2 with points pt 7 ps 1.5 title ''")?;

    // Output data for impulses
    for event in &events {
        let start = f64::from(event.part.start());
        let sample = event.value.sample();
        let y_idx = sample_names.iter().position(|n| n == sample).unwrap() + 1;
        writeln!(file, "{start:.3} {y_idx}")?;
    }
    writeln!(file, "e")?;

    // Output data for points
    for event in &events {
        let start = f64::from(event.part.start());
        let sample = event.value.sample();
        let y_idx = sample_names.iter().position(|n| n == sample).unwrap() + 1;
        writeln!(file, "{start:.3} {y_idx}")?;
    }
    writeln!(file, "e")?;

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
    fn gnuplot_exporter_generates_number_pattern() {
        let source = "melody = 0 12";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("melody").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join(format!("test_gnuplot_number_{}.plt", unique_temp_suffix()));
        export_number_pattern_to_gnuplot(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("set title 'Orpheus Number Pattern'"));
        assert!(content.contains("0.000 0.000"));
        assert!(content.contains("0.500 12.000"));

        // Clean up
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn gnuplot_exporter_generates_sample_pattern() {
        let source = "drums = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("drums").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join(format!("test_gnuplot_sample_{}.plt", unique_temp_suffix()));
        export_sample_pattern_to_gnuplot(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("set title 'Orpheus Sample Pattern'"));
        assert!(content.contains("set ytics ('bd' 1, 'sn' 2)"));
        assert!(content.contains("0.000 1"));
        assert!(content.contains("0.500 2"));

        // Clean up
        let _ = std::fs::remove_file(&path);
    }
}
