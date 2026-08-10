//! The `bash_export` module provides an exporter to Bash scripts.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

const SECONDS_PER_CYCLE: f64 = 2.0;

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
    writeln!(file, "#!/bin/bash")?;
    writeln!(file, "# Orpheus Bash Export")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    let mut current_time = 0.0;
    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(file, "sleep {sleep_dur:.3}")?;
            current_time = start_time;
        }
        writeln!(file, "echo \"play sample: {}\"", event.value.sample())?;
    }
    Ok(())
}

pub fn export_number_pattern_to_bash(
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
    writeln!(file, "#!/bin/bash")?;
    writeln!(file, "# Orpheus Bash Export")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    let mut current_time = 0.0;
    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        if start_time > current_time {
            let sleep_dur = start_time - current_time;
            writeln!(file, "sleep {sleep_dur:.3}")?;
            current_time = start_time;
        }
        writeln!(file, "echo \"play note: {:.3}\"", event.value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{ReplMode, eval_module};

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
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();
        assert_eq!(
            super::export_number_pattern_to_bash(pat, "test.sh", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
