//! The `css_export` module provides an exporter to CSS `@keyframes`.
//!
//! This exporter generates a `.css` file containing animation keyframes
//! mapped from number patterns or sample patterns. This allows driving
//! web animations directly from Orpheus patterns.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Exports a number pattern's evaluated events to a CSS file as `@keyframes`.
///
/// Number patterns are mapped to a CSS custom property `--orpheus-value`.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_number_pattern_to_css};
///
/// let env = eval_module("x = 0.5 1.0 0.2", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.css");
/// export_number_pattern_to_css(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(clippy::cast_precision_loss)]
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
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "/* Orpheus CSS Export */")?;
    writeln!(file, "/* Cycles: {cycle_count} */")?;
    writeln!(file, ":root {{")?;
    writeln!(
        file,
        "  --orpheus-duration: {}s;",
        (cycle_count as f64) * 2.0
    )?;
    writeln!(file, "}}")?;
    writeln!(file)?;

    writeln!(file, "@keyframes orpheus-number-pattern {{")?;

    let total_cycles_f64 = cycle_count as f64;

    for event in events {
        let start_time = f64::from(event.part.start());
        let percent = (start_time / total_cycles_f64) * 100.0;
        let value = event.value;

        // Use step-end or similar to hold value until next? Or just keyframes.
        // For simplicity, just output percentages.
        writeln!(file, "  {percent:.2}% {{ --orpheus-value: {value:.3}; }}")?;
    }

    writeln!(file, "  100% {{}}")?;
    writeln!(file, "}}")?;
    writeln!(file)?;

    writeln!(file, ".orpheus-animated-number {{")?;
    writeln!(
        file,
        "  animation: orpheus-number-pattern var(--orpheus-duration) linear infinite;"
    )?;
    writeln!(file, "}}")?;

    Ok(())
}

/// Exports a sample pattern's evaluated events to a CSS file.
///
/// Each sample gets its own `@keyframes` animation, which spikes to `opacity: 1`
/// and `transform: scale(1.2)` when played, and decays immediately after.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module, export_sample_pattern_to_css};
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export_sample.css");
/// export_sample_pattern_to_css(pattern, &path, 1).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(clippy::cast_precision_loss)]
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

    let mut samples = BTreeSet::new();
    for event in &events {
        samples.insert(event.value.sample().to_string());
    }

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    writeln!(file, "/* Orpheus CSS Export */")?;
    writeln!(file, "/* Cycles: {cycle_count} */")?;
    writeln!(file, ":root {{")?;
    writeln!(
        file,
        "  --orpheus-duration: {}s;",
        (cycle_count as f64) * 2.0
    )?;
    writeln!(file, "}}")?;
    writeln!(file)?;

    let total_cycles_f64 = cycle_count as f64;

    for sample in samples {
        writeln!(file, "@keyframes orpheus-sample-{sample} {{")?;

        // Find events for this sample
        let sample_events: Vec<_> = events
            .iter()
            .filter(|e| e.value.sample() == sample)
            .collect();

        let mut last_percent = -1.0;

        for event in sample_events {
            let start_time = f64::from(event.part.start());
            let percent = (start_time / total_cycles_f64) * 100.0;

            // Just before hit
            if percent > 0.0 && percent - 0.1 > last_percent {
                writeln!(
                    file,
                    "  {:.2}% {{ opacity: 0; transform: scale(1.0); }}",
                    percent - 0.1
                )?;
            }
            // The hit
            let gain = event.value.gain();
            let scale = gain.mul_add(0.5, 1.0); // 1.0 to 1.5 based on gain
            writeln!(
                file,
                "  {percent:.2}% {{ opacity: {gain:.2}; transform: scale({scale:.2}); }}"
            )?;

            // Quick decay
            let decay_percent = percent + 5.0; // arbitrary decay length
            if decay_percent <= 100.0 {
                writeln!(
                    file,
                    "  {decay_percent:.2}% {{ opacity: 0; transform: scale(1.0); }}"
                )?;
                last_percent = decay_percent;
            } else {
                last_percent = percent;
            }
        }

        writeln!(file, "  100% {{ opacity: 0; transform: scale(1.0); }}")?;
        writeln!(file, "}}")?;
        writeln!(file)?;

        writeln!(file, ".orpheus-animated-sample-{sample} {{")?;
        writeln!(
            file,
            "  animation: orpheus-sample-{sample} var(--orpheus-duration) linear infinite;"
        )?;
        writeln!(file, "  opacity: 0; /* default state */")?;
        writeln!(file, "}}")?;
        writeln!(file)?;
    }

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
    fn css_exporter_generates_number_pattern() {
        let source = "x = 0 0.5 1";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_css_num_output_{}.css", unique_temp_suffix()));
        export_number_pattern_to_css(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("@keyframes orpheus-number-pattern"));
        assert!(content.contains("--orpheus-value: 0.000"));
        assert!(content.contains("--orpheus-value: 0.500"));
        assert!(content.contains("--orpheus-value: 1.000"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn css_exporter_generates_sample_pattern() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join(format!(
            "test_css_sample_output_{}.css",
            unique_temp_suffix()
        ));
        export_sample_pattern_to_css(pattern, &path, 1).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("@keyframes orpheus-sample-bd"));
        assert!(content.contains("@keyframes orpheus-sample-sn"));
        assert!(content.contains(".orpheus-animated-sample-bd"));

        let _ = fs::remove_file(path);
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

        let module2 = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat2 = module2.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_css(pat2, "test.css", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
