//! The `css_export` module provides an exporter to CSS animations.
//!
//! This exporter generates a `.css` file containing `@keyframes` and CSS classes
//! that synchronize web animations to Orpheus patterns.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// 1 cycle = 2.0 seconds
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a sample pattern's evaluated events to CSS keyframes.
///
/// Each unique sample generates a CSS class (e.g., `.orpheus-sample-bd`) that runs
/// an animation scaling up and down in sync with the pattern triggers.
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails or if `cycle_count` is 0.
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
    events.sort_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    let total_time = (cycle_count as f64) * SECONDS_PER_CYCLE;

    writeln!(file, "/* Orpheus CSS Sample Export */")?;
    writeln!(file, "/* Cycles: {cycle_count} ({total_time}s) */")?;
    writeln!(file)?;

    // Group events by sample
    let mut samples: std::collections::BTreeMap<&str, Vec<_>> = std::collections::BTreeMap::new();
    for event in &events {
        samples.entry(event.value.sample()).or_default().push(event);
    }

    for (sample, sample_events) in samples {
        let safe_name = sample.replace(|c: char| !c.is_ascii_alphanumeric(), "-");
        let anim_name = format!("orpheus-anim-{safe_name}");

        writeln!(file, ".orpheus-sample-{safe_name} {{")?;
        writeln!(
            file,
            "  animation: {anim_name} {total_time}s linear infinite;"
        )?;
        writeln!(file, "}}")?;
        writeln!(file)?;

        writeln!(file, "@keyframes {anim_name} {{")?;

        for event in sample_events {
            let start_pct = (f64::from(event.part.start()) / (cycle_count as f64)) * 100.0;
            let decay_pct = (start_pct + 2.0).min(100.0);

            writeln!(
                file,
                "  {start_pct:.2}% {{ transform: scale(1.2); opacity: 1; }}"
            )?;
            writeln!(
                file,
                "  {decay_pct:.2}% {{ transform: scale(1.0); opacity: 0.5; }}"
            )?;
        }
        writeln!(file, "}}")?;
        writeln!(file)?;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to CSS variable animations.
///
/// Generates a keyframe animation that steps through the numeric values of the pattern
/// using a CSS custom property `--orpheus-value`.
///
/// # Errors
/// Returns [`EvalError`] if pattern querying fails or if `cycle_count` is 0.
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
    events.sort_by(|a, b| a.part.start().cmp(b.part.start()));

    let path = path.as_ref();
    let mut file = std::fs::File::create(path).map_err(|e| EvalError::new(e.to_string()))?;

    let total_time = (cycle_count as f64) * SECONDS_PER_CYCLE;

    writeln!(file, "/* Orpheus CSS Number Export */")?;
    writeln!(file, "/* Cycles: {cycle_count} ({total_time}s) */")?;
    writeln!(file)?;

    writeln!(file, ".orpheus-number {{")?;
    writeln!(
        file,
        "  animation: orpheus-number-anim {total_time}s step-start infinite;"
    )?;
    writeln!(file, "}}")?;
    writeln!(file)?;

    writeln!(file, "@keyframes orpheus-number-anim {{")?;
    for event in events {
        let start_pct = (f64::from(event.part.start()) / (cycle_count as f64)) * 100.0;
        writeln!(
            file,
            "  {start_pct:.2}% {{ --orpheus-value: {:.3}; }}",
            event.value
        )?;
    }
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
    fn css_exporter_generates_sample_animation() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_css_sample_{}.css", unique_temp_suffix()));
        export_sample_pattern_to_css(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("/* Orpheus CSS Sample Export */"));
        assert!(content.contains(".orpheus-sample-bd {"));
        assert!(content.contains("@keyframes orpheus-anim-bd {"));
        assert!(content.contains("0.00% { transform: scale(1.2); opacity: 1; }"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn css_exporter_generates_number_animation() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_css_number_{}.css", unique_temp_suffix()));
        export_number_pattern_to_css(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("/* Orpheus CSS Number Export */"));
        assert!(content.contains(".orpheus-number {"));
        assert!(content.contains("@keyframes orpheus-number-anim {"));
        assert!(content.contains("0.00% { --orpheus-value: 60.000; }"));
        assert!(content.contains("33.33% { --orpheus-value: 62.000; }"));
        assert!(content.contains("66.67% { --orpheus-value: 64.000; }"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat_num = module.get("pat").unwrap().as_number_pattern().unwrap();

        let module_samp = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat_samp = module_samp.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_css(pat_num, "test.css", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        assert_eq!(
            super::export_sample_pattern_to_css(pat_samp, "test.css", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
