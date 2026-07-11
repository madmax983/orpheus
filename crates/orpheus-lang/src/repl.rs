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
    let mut session = ReplSession::with_engine(engine);

    let mut stdout = stdout.lock();
    let mut stderr = stderr.lock();

    if let Some(msg) = warning {
        let mut lines = msg.lines();
        if let Some(first) = lines.next() {
            writeln!(
                stderr,
                "{} {}",
                "[Warn]".yellow().bold(),
                first.yellow().bold()
            )?;
            for line in lines {
                writeln!(stderr, "{}", line.dark_grey())?;
            }
        }
    }

    if let Some(path) = startup_path {
        match session.open_file(path) {
            Ok(msg) => writeln!(stdout, "{} {}", "\u{2713}".green(), msg.green())?,
            Err(msg) => {
                let mut lines = msg.lines();
                if let Some(first) = lines.next() {
                    writeln!(stderr, "{} {}", "\u{2717}".red().bold(), first.red().bold())?;
                    for line in lines {
                        writeln!(stderr, "{}", line.dark_grey())?;
                    }
                }
            }
        }
    }

    run_with_handles(stdin.lock(), stdout, stderr, &mut session)
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
            Ok(message) => writeln!(stdout, "{} {}", "\u{2713}".green(), message.green())?,
            Err(message) => {
                let mut lines = message.lines();
                if let Some(first) = lines.next() {
                    writeln!(stderr, "{} {}", "\u{2717}".red().bold(), first.red().bold())?;
                    for line in lines {
                        writeln!(stderr, "{}", line.dark_grey())?;
                    }
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

        run_with_handles(reader, &mut stdout, &mut stderr, &mut session).unwrap();

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

        run_with_handles(reader, &mut stdout, &mut stderr, &mut session).unwrap();

        let stderr_str = String::from_utf8(stderr).unwrap();
        assert!(stderr_str.contains("\u{2717} parse error"));
    }
}
