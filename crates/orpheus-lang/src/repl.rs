use crossterm::style::Stylize;
use std::io::{self, BufRead, Write};
use std::path::Path;

use orpheus_dsp::EngineHandle;

use crate::session::ReplSession;

/// Runs the phase-one Orpheus REPL.
///
/// This provides a line-based terminal interface for evaluating patterns
/// sequentially against a persistent evaluation context.
///
/// Blank lines are ignored. `:quit` exits the session.
///
/// # Examples
///
/// ```no_run
/// use orpheus_lang::repl::run_stdio;
///
/// // run_stdio().unwrap(); // Blocks main thread waiting for stdin
/// ```
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
/// # Examples
///
/// ```no_run
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::repl::run_stdio_with_engine;
///
/// let engine = EngineHandle::stub();
/// // run_stdio_with_engine(engine).unwrap();
/// ```
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
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use orpheus_dsp::EngineHandle;
/// use orpheus_lang::repl::run_stdio_with_engine_and_path;
///
/// let engine = EngineHandle::stub();
/// let start_file = Path::new("init.ode");
/// // run_stdio_with_engine_and_path(engine, Some(start_file), None).unwrap();
/// ```
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
        writeln!(stderr, "{}", format!("⚠️ {msg}").yellow().bold())?;
    }

    if let Some(path) = startup_path {
        match session.open_file(path) {
            Ok(msg) => writeln!(stdout, "{}", format!("✓ {msg}").green())?,
            Err(msg) => writeln!(stderr, "{}", format!("✗ {msg}").red().bold())?,
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
            Ok(message) => writeln!(stdout, "{}", format!("✓ {message}").green())?,
            Err(message) => writeln!(stderr, "{}", format!("✗ {message}").red().bold())?,
        }
    }

    Ok(())
}
