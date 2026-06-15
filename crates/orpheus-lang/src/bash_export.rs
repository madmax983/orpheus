//! The `bash_export` module provides an exporter to executable Bash scripts.
//!
//! This exporter translates evaluated sample patterns and number patterns
//! into an executable Bash script that sleeps for the calculated duration and
//! then prints or "plays" the corresponding values. It brings Orpheus sequences
//! directly into standard shell pipelines.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

// Assume 120 BPM, 4 beats per cycle -> 1 cycle = 2.0 seconds
const SECONDS_PER_CYCLE: f64 = 2.0;

/// Exports a sample pattern's evaluated events to an executable Bash script.
///
/// The generated script uses `sleep` commands to handle the timing and `echo`
/// to output the sample names to stdout.
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
/// let path = std::env::temp_dir().join("play.sh");
/// export_sample_pattern_to_bash(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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

    // Make the file executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file
            .metadata()
            .map_err(|e| EvalError::new(e.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        file.set_permissions(perms)
            .map_err(|e| EvalError::new(e.to_string()))?;
    }

    writeln!(file, "#!/usr/bin/env bash")?;
    writeln!(file, "# Orpheus Bash Export")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(file)?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let sample = event.value.sample();

        if start_time > current_time {
            let sleep_duration = start_time - current_time;
            writeln!(file, "sleep {sleep_duration:.3}")?;
        }

        writeln!(file, "echo \"{sample}\"")?;
        current_time = start_time;
    }

    Ok(())
}

/// Exports a number pattern's evaluated events to an executable Bash script.
///
/// The generated script uses `sleep` commands to handle the timing and `echo`
/// to output the evaluated numeric values to stdout.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{ReplMode, eval_module};
/// use orpheus_lang::export_number_pattern_to_bash;
///
/// let env = eval_module("x = 1 2 3", ReplMode::Loose).unwrap();
/// let pattern = env.get("x").unwrap().as_number_pattern().unwrap();
///
/// let path = std::env::temp_dir().join("numbers.sh");
/// export_number_pattern_to_bash(pattern, &path, 2).unwrap();
/// ```
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
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

    // Make the file executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file
            .metadata()
            .map_err(|e| EvalError::new(e.to_string()))?
            .permissions();
        perms.set_mode(0o755);
        file.set_permissions(perms)
            .map_err(|e| EvalError::new(e.to_string()))?;
    }

    writeln!(file, "#!/usr/bin/env bash")?;
    writeln!(file, "# Orpheus Bash Export")?;
    writeln!(file, "# Cycles: {cycle_count}")?;
    writeln!(file)?;

    let mut current_time = 0.0;

    for event in events {
        let start_time = f64::from(event.part.start()) * SECONDS_PER_CYCLE;
        let value = event.value;

        if start_time > current_time {
            let sleep_duration = start_time - current_time;
            writeln!(file, "sleep {sleep_duration:.3}")?;
        }

        writeln!(file, "echo \"{value:.6}\"")?;
        current_time = start_time;
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
    fn bash_exporter_generates_script_for_sample_pattern() {
        let source = "pattern = bd sn";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_bash_output_{}.sh", unique_temp_suffix()));
        export_sample_pattern_to_bash(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("#!/usr/bin/env bash"));
        assert!(content.contains("# Orpheus Bash Export"));
        assert!(content.contains("echo \"bd\""));
        assert!(content.contains("sleep 1.000")); // bd is at 0, sn is at 1.0s
        assert!(content.contains("echo \"sn\""));
    }

    #[test]
    fn bash_exporter_generates_script_for_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path =
            std::env::temp_dir().join(format!("test_bash_num_output_{}.sh", unique_temp_suffix()));
        export_number_pattern_to_bash(pattern, &path, 1).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("#!/usr/bin/env bash"));
        assert!(content.contains("echo \"60.000000\""));
        // 1/3 of a 2.0s cycle is ~0.667s
        assert!(content.contains("sleep 0.667"));
        assert!(content.contains("echo \"62.000000\""));
        assert!(content.contains("echo \"64.000000\""));
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let module = eval_module("pat = 1 2", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();

        assert_eq!(
            super::export_number_pattern_to_bash(pat, "test.sh", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );

        let module = eval_module("pat = bd", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();

        assert_eq!(
            super::export_sample_pattern_to_bash(pat, "test.sh", 0)
                .unwrap_err()
                .to_string(),
            "exporting requires at least one cycle"
        );
    }
}
