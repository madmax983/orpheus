//! The `bash_export` module provides an exporter to Bash scripts.
//!
//! This exporter generates a `.sh` Bash script that echoes
//! the samples triggered in an evaluated Orpheus pattern, using
//! the `sleep` command to schedule the outputs correctly.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::SamplePatternValue;

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a sample pattern's evaluated events to a Bash script.
///
/// Each line in the generated script echoes the sample name
/// and uses `sleep` to match the exact time of the event.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_sample_pattern_to_bash;
///
/// let env = eval_module("x = bd sn", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_sample_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("export.sh");
/// export_sample_pattern_to_bash(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
pub fn export_sample_pattern_to_bash(
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

    writeln!(file, "#!/usr/bin/env bash").map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(file, "# Orpheus Bash Script Export").map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(file, "# ==========================").map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(file, "# Cycles: {cycle_count}").map_err(|e| EvalError::new(e.to_string()))?;
    writeln!(file).map_err(|e| EvalError::new(e.to_string()))?;

    let mut current_time = 0.0;

    for event in &events {
        let sample = event.value.sample();
        let target_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let sleep_duration = target_time - current_time;

        if sleep_duration > 0.0 {
            writeln!(file, "sleep {sleep_duration:.3}").map_err(|e| EvalError::new(e.to_string()))?;
            current_time = target_time;
        }

        writeln!(file, "echo '[+] Trigger: {sample}'").map_err(|e| EvalError::new(e.to_string()))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn bash_exporter_generates_valid_script() {
        let source = "x = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("x").unwrap().as_sample_pattern().unwrap();

        let path = std::env::temp_dir().join("test_bash_export.sh");
        export_sample_pattern_to_bash(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("#!/usr/bin/env bash"));
        assert!(content.contains("echo '[+] Trigger: bd'"));
        assert!(content.contains("sleep 1.000"));
        assert!(content.contains("echo '[+] Trigger: sn'"));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = bd sn", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_bash(pat, "test.sh", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
