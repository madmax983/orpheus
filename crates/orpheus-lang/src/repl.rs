//! The `repl` module provides a simple standard I/O command-line interface.
//!
//! This module implements a basic interactive read-eval-print loop (REPL) that
//! reads user input line-by-line from `stdin`, evaluates it against a running
//! `ReplSession`, and prints the formatted results or errors to `stdout`.

use crossterm::style::Stylize;
use std::io::{self, BufRead, Write};
use std::path::Path;

use orpheus_dsp::EngineHandle;

use crate::session::ReplSession;

///
/// Blank lines are ignored. `:quit` exits the session.
///
/// # Errors
///
/// Returns any terminal I/O failure encountered while reading input or
/// writing REPL output.
pub fn run_stdio() -> io::Result<()> {
    run_stdio_with_engine(EngineHandle::stub())
}

/// Runs the phase-one Orpheus REPL with the provided audio engine handle.
///
/// Blank lines are ignored. `:quit` exits the session.
///
/// # Errors
///
/// Returns any terminal I/O failure encountered while reading input or
/// writing REPL output.
pub fn run_stdio_with_engine(engine: EngineHandle) -> io::Result<()> {
    run_stdio_with_engine_and_path(engine, None, None)
}

/// Runs the phase-one Orpheus REPL with an optional startup `.ode` preload.
///
/// # Errors
///
/// Returns startup file load failures or terminal I/O failures encountered
/// while the REPL is active.
pub fn run_stdio_with_engine_and_path(
    engine: EngineHandle,
    startup_path: Option<&Path>,
    warning: Option<String>,
) -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();

    init_session_and_run_with_handles(
        engine,
        startup_path,
        warning,
        stdin.lock(),
        stdout.lock(),
        stderr.lock(),
    )
}

fn init_session_and_run_with_handles<R, W, E>(
    engine: EngineHandle,
    startup_path: Option<&Path>,
    warning: Option<String>,
    reader: R,
    mut stdout: W,
    mut stderr: E,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
    E: Write,
{
    let mut session = ReplSession::with_engine(engine);

    if let Some(msg) = warning {
        writeln!(stderr, "{}", format!("⚠️ {msg}").yellow().bold())?;
    }

    if let Some(path) = startup_path {
        match session.open_file(path) {
            Ok(msg) => writeln!(stdout, "{}", format!("✓ {msg}").green())?,
            Err(msg) => writeln!(stderr, "{}", format!("✗ {msg}").red().bold())?,
        }
    }

    run_with_handles(reader, stdout, stderr, &mut session)
}

fn run_with_handles<R, W, E>(
    mut reader: R,
    mut stdout: W,
    mut stderr: E,
    session: &mut ReplSession,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
    E: Write,
{
    let mut line = String::new();
    loop {
        write!(stdout, "{}", "> ".dark_grey())?;
        stdout.flush()?;
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == ":quit" {
            break;
        }

        match session.eval_line(trimmed) {
            Ok(message) => writeln!(stdout, "{}", format!("✓ {message}").green())?,
            Err(message) => writeln!(stderr, "{}", format!("✗ {message}").red().bold())?,
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_with_handles_evaluates_lines_and_quits() {
        let input = "a = bd sn\n\n:quit\n";
        let reader = std::io::Cursor::new(input);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut session = ReplSession::with_engine(EngineHandle::stub());

        run_with_handles(reader, &mut stdout, &mut stderr, &mut session).unwrap();

        let stdout_str = String::from_utf8(stdout).unwrap();
        let stderr_str = String::from_utf8(stderr).unwrap();

        assert!(stdout_str.contains("✓ bound a"));
        assert_eq!(stderr_str, "");
    }

    #[test]
    fn run_with_handles_reports_errors_to_stderr() {
        let input = "a = \n:quit\n";
        let reader = std::io::Cursor::new(input);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut session = ReplSession::with_engine(EngineHandle::stub());

        run_with_handles(reader, &mut stdout, &mut stderr, &mut session).unwrap();

        let stderr_str = String::from_utf8(stderr).unwrap();
        assert!(stderr_str.contains("✗ parse error"));
    }

    #[test]
    fn init_session_and_run_with_handles_prints_warning() {
        let input = ":quit\n";
        let reader = std::io::Cursor::new(input);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        init_session_and_run_with_handles(
            EngineHandle::stub(),
            None,
            Some("test warning".to_string()),
            reader,
            &mut stdout,
            &mut stderr,
        )
        .unwrap();

        let stderr_str = String::from_utf8(stderr).unwrap();
        assert!(stderr_str.contains("⚠️ test warning"));
    }

    #[test]
    fn init_session_and_run_with_handles_loads_file_successfully() {
        use std::io::Write;

        let dir = std::env::temp_dir();
        let file_path = dir.join("test_startup.ode");
        let mut file = std::fs::File::create(&file_path).unwrap();
        writeln!(file, "a = bd").unwrap();

        let input = ":quit\n";
        let reader = std::io::Cursor::new(input);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        init_session_and_run_with_handles(
            EngineHandle::stub(),
            Some(&file_path),
            None,
            reader,
            &mut stdout,
            &mut stderr,
        )
        .unwrap();

        let stdout_str = String::from_utf8(stdout).unwrap();
        assert!(
            stdout_str.contains("✓ loaded")
                || stdout_str.contains("✓ evaluated")
                || stdout_str.contains("✓ opened"),
            "stdout was: {}",
            stdout_str
        );
    }

    #[test]
    fn init_session_and_run_with_handles_reports_file_load_error() {
        let file_path = Path::new("/path/that/does/not/exist.ode");
        let input = ":quit\n";
        let reader = std::io::Cursor::new(input);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        init_session_and_run_with_handles(
            EngineHandle::stub(),
            Some(file_path),
            None,
            reader,
            &mut stdout,
            &mut stderr,
        )
        .unwrap();

        let stderr_str = String::from_utf8(stderr).unwrap();
        assert!(
            stderr_str.contains("✗ failed to read file") || stderr_str.contains("✗"),
            "stderr was: {}",
            stderr_str
        );
    }
}
