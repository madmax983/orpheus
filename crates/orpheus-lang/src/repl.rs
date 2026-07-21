//! The `repl` module provides a simple standard I/O command-line interface.
//!
//! This module implements a basic interactive read-eval-print loop (REPL) that
//! reads user input line-by-line from `stdin`, evaluates it against a running
//! `ReplSession`, and prints the formatted results or errors to `stdout`.

use crossterm::style::Stylize;
use std::io::{self, BufRead, IsTerminal, Write};
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
    let mut session = ReplSession::with_engine(engine);

    let stdout_is_terminal = stdout.is_terminal();
    let stderr_is_terminal = stderr.is_terminal();

    let mut stdout = stdout.lock();
    let mut stderr = stderr.lock();

    if let Some(msg) = warning {
        if stderr_is_terminal {
            writeln!(stderr, "{}", format!("[Warn] {msg}").yellow().bold())?;
        } else {
            writeln!(stderr, "[Warn] {msg}")?;
        }
    }

    if let Some(path) = startup_path {
        match session.open_file(path) {
            Ok(msg) => {
                if stdout_is_terminal {
                    writeln!(stdout, "{}", format!("\u{2713} {msg}").green())?;
                } else {
                    writeln!(stdout, "\u{2713} {msg}")?;
                }
            }
            Err(msg) => {
                if stderr_is_terminal {
                    writeln!(stderr, "{}", format!("\u{2717} {msg}").red().bold())?;
                } else {
                    writeln!(stderr, "\u{2717} {msg}")?;
                }
            }
        }
    }

    run_with_handles(
        stdin.lock(),
        stdout,
        stderr,
        &mut session,
        stdout_is_terminal,
        stderr_is_terminal,
    )
}

fn run_with_handles<R, W, E>(
    mut reader: R,
    mut stdout: W,
    mut stderr: E,
    session: &mut ReplSession,
    stdout_is_terminal: bool,
    stderr_is_terminal: bool,
) -> io::Result<()>
where
    R: BufRead,
    W: Write,
    E: Write,
{
    let mut line = String::new();
    loop {
        if stdout_is_terminal {
            write!(stdout, "{}", "> ".dark_grey())?;
        } else {
            write!(stdout, "> ")?;
        }
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
            Ok(message) => {
                if stdout_is_terminal {
                    writeln!(stdout, "{}", format!("\u{2713} {message}").green())?;
                } else {
                    writeln!(stdout, "\u{2713} {message}")?;
                }
            }
            Err(message) => {
                if stderr_is_terminal {
                    writeln!(stderr, "{}", format!("\u{2717} {message}").red().bold())?;
                } else {
                    writeln!(stderr, "\u{2717} {message}")?;
                }
            }
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

        run_with_handles(reader, &mut stdout, &mut stderr, &mut session, false, false).unwrap();

        let stdout_str = String::from_utf8(stdout).unwrap();
        let stderr_str = String::from_utf8(stderr).unwrap();

        assert!(stdout_str.contains("\u{2713} bound a"));
        assert_eq!(stderr_str, "");
    }

    #[test]
    fn run_with_handles_reports_errors_to_stderr() {
        let input = "a = \n:quit\n";
        let reader = std::io::Cursor::new(input);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut session = ReplSession::with_engine(EngineHandle::stub());

        run_with_handles(reader, &mut stdout, &mut stderr, &mut session, false, false).unwrap();

        let stderr_str = String::from_utf8(stderr).unwrap();
        assert!(stderr_str.contains("\u{2717} parse error"));
    }
}
