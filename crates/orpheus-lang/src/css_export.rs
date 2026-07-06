//! The `css_export` module provides export functionality for CSS animations.
//!
//! This exporter visualizes evaluated patterns as CSS `@keyframes` blocks,
//! which can be used to drive web animations synced to the patterns.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a sample pattern's evaluated events to CSS `@keyframes`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_sample_pattern_to_css;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("drums.css");
/// export_sample_pattern_to_css(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
pub fn export_sample_pattern_to_css(
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

    writeln!(file, "/* Orpheus Sample Pattern CSS Export */")?;
    writeln!(file, "/* Cycles: {cycle_count} */\n")?;

    let mut samples: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for event in &events {
        samples.insert(event.value.sample());
    }

    for sample in samples {
        writeln!(file, "@keyframes orpheus_sample_{sample} {{")?;

        let mut last_percentage = -1.0;

        for event in &events {
            if event.value.sample() != sample {
                continue;
            }
            let start = f64::from(event.part.start());
            let end = f64::from(event.part.end());

            let start_pct = (start / (cycle_count as f64)) * 100.0;
            let end_pct = (end / (cycle_count as f64)) * 100.0;

            if start_pct > last_percentage + 0.001 && last_percentage >= 0.0 {
                writeln!(
                    file,
                    "  {:.3}% {{ opacity: 0; transform: scale(1); }}",
                    start_pct - 0.001
                )?;
            }
            writeln!(
                file,
                "  {start_pct:.3}% {{ opacity: 1; transform: scale(1.1); }}"
            )?;
            writeln!(
                file,
                "  {end_pct:.3}% {{ opacity: 0; transform: scale(1); }}"
            )?;

            last_percentage = end_pct;
        }

        writeln!(file, "}}\n")?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to CSS `@keyframes`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_css};
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("automation.css");
/// export_number_pattern_to_css(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
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
    events.sort_unstable_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path)?;

    writeln!(file, "/* Orpheus Number Pattern CSS Export */")?;
    writeln!(file, "/* Cycles: {cycle_count} */\n")?;

    writeln!(file, "@keyframes orpheus_number_pattern {{")?;

    for event in &events {
        let start = f64::from(event.part.start());
        let val = event.value;
        let start_pct = (start / (cycle_count as f64)) * 100.0;

        writeln!(file, "  {start_pct:.3}% {{ --orpheus-val: {val:.3}; }}")?;
    }

    writeln!(file, "}}")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};
    use std::fs;
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
    fn css_exporter_generates_sample_pattern() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join(format!("test_output_{}.css", unique_temp_suffix()));
        export_sample_pattern_to_css(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("@keyframes orpheus_sample_bd {"));
        assert!(content.contains("@keyframes orpheus_sample_sn {"));
        assert!(content.contains("0.000% { opacity: 1; transform: scale(1.1); }"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn css_exporter_generates_number_pattern() {
        let source = "x = 1 2";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_output_num_{}.css", unique_temp_suffix()));
        export_number_pattern_to_css(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("@keyframes orpheus_number_pattern {"));
        assert!(content.contains("--orpheus-val: 1.000;"));
        assert!(content.contains("--orpheus-val: 2.000;"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();
        assert_eq!(
            export_sample_pattern_to_css(pat, "test.css", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();
        assert_eq!(
            export_number_pattern_to_css(pat, "test.css", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
