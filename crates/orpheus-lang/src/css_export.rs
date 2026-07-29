//! The `css_export` module provides an exporter to CSS keyframes.
//!
//! This exporter generates a `.css` file containing keyframe animations
//! corresponding to evaluated pattern events, mapping pitch and duration
//! into CSS variables and animation blocks.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds = 2000 ms.
const MS_PER_CYCLE: f64 = 2000.0;

/// Exports a number pattern's evaluated events to a CSS stylesheet.
///
/// Number patterns are assumed to represent pitch/trigger. Events are
/// converted into CSS `@keyframes` that can be applied to DOM elements
/// to create visual animations perfectly synced with the musical pattern.
///
/// # Examples
///
/// ```no_run
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_css;
///
/// let env = eval_module("x = 60 62 64", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_animation.css");
/// export_number_pattern_to_css(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_number_pattern_to_css(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let mut events = pattern.try_query(&span)?;
    events.sort_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    #[allow(clippy::cast_precision_loss)]
    let total_time_ms = (cycle_count as f64) * MS_PER_CYCLE;

    writeln!(file, "/* Orpheus CSS Animation Export */")?;
    writeln!(file, "/* ============================== */")?;
    writeln!(
        file,
        "/* Cycles: {cycle_count} (Total Time: {total_time_ms}ms) */"
    )?;
    writeln!(file)?;

    writeln!(file, ":root {{")?;
    writeln!(file, "  --orpheus-duration: {total_time_ms}ms;")?;
    writeln!(file, "}}")?;
    writeln!(file)?;

    writeln!(file, "@keyframes orpheus-pattern {{")?;

    for event in &events {
        let start_ms = f64::from(event.part.start()) * MS_PER_CYCLE;
        let end_ms = f64::from(event.part.end()) * MS_PER_CYCLE;

        let start_pct = (start_ms / total_time_ms) * 100.0;
        let end_pct = (end_ms / total_time_ms) * 100.0;

        let val = event.value;

        // Turn event on
        writeln!(file, "  {start_pct:.2}% {{")?;
        writeln!(file, "    opacity: 1;")?;
        writeln!(
            file,
            "    transform: scale(1.1) translateY({:.1}px);",
            val - 60.0
        )?;
        writeln!(file, "    --orpheus-val: {val:.2};")?;
        writeln!(file, "  }}")?;

        // Turn event off right before the next step (or at the exact end of its duration)
        let end_time = end_pct - 0.01;
        writeln!(file, "  {end_time:.2}% {{")?;
        writeln!(file, "    opacity: 0.5;")?;
        writeln!(file, "    transform: scale(1.0) translateY(0px);")?;
        writeln!(file, "  }}")?;
    }

    // Ensure it ends cleanly
    writeln!(file, "  100% {{")?;
    writeln!(file, "    opacity: 0.5;")?;
    writeln!(file, "    transform: scale(1.0) translateY(0px);")?;
    writeln!(file, "  }}")?;
    writeln!(file, "}}")?;
    writeln!(file)?;

    writeln!(file, ".orpheus-animated {{")?;
    writeln!(
        file,
        "  animation: orpheus-pattern var(--orpheus-duration) linear infinite;"
    )?;
    writeln!(file, "}}")?;

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
    fn css_exporter_generates_keyframes() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_css_output_{}.css", unique_temp_suffix()));
        export_number_pattern_to_css(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("/* Orpheus CSS Animation Export */"));
        assert!(content.contains("--orpheus-duration: 2000ms;"));
        assert!(content.contains("@keyframes orpheus-pattern"));
        assert!(content.contains("transform: scale(1.1) translateY(0.0px);"));
        assert!(
            content.contains("animation: orpheus-pattern var(--orpheus-duration) linear infinite;")
        );

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_css(pat, "test.css", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
