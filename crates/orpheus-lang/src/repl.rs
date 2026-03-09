use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

use orpheus_dsp::{EngineCommand, EngineHandle, PatternUpdate};

use crate::eval::eval_into_bindings;
use crate::types::infer_into_bindings;
use crate::{ReplMode, Type, Value};

/// Runs the phase-one Orpheus REPL over standard input and output.
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
    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut session = ReplSession::with_engine(engine);

    run_with_handles(stdin.lock(), stdout.lock(), stderr.lock(), &mut session)
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
            Ok(message) => writeln!(stdout, "{message}")?,
            Err(message) => writeln!(stderr, "{message}")?,
        }
    }

    Ok(())
}

pub(crate) struct ReplSession {
    mode: ReplMode,
    engine: EngineHandle,
    bindings: BTreeMap<String, Value>,
    type_bindings: BTreeMap<String, Type>,
}

impl ReplSession {
    #[cfg(test)]
    fn new() -> Self {
        Self::with_engine(EngineHandle::stub())
    }

    pub(crate) const fn with_engine(engine: EngineHandle) -> Self {
        Self {
            mode: ReplMode::Loose,
            engine,
            bindings: BTreeMap::new(),
            type_bindings: BTreeMap::new(),
        }
    }

    pub(crate) fn eval_line(&mut self, source: &str) -> Result<String, String> {
        let Some((name, ty)) = infer_into_bindings(source, self.mode, &mut self.type_bindings)
            .map_err(|error| error.to_string())?
        else {
            return Err("no bindings were produced".into());
        };
        let Some((value_name, value)) = eval_into_bindings(source, self.mode, &mut self.bindings)
            .map_err(|error| error.to_string())?
        else {
            return Err("no bindings were produced".into());
        };
        debug_assert_eq!(name, value_name);

        self.push_pattern_update(&name, &value)?;
        Ok(success_banner(&ty))
    }

    fn push_pattern_update(&mut self, name: &str, value: &Value) -> Result<(), String> {
        if let Value::SamplePattern(pattern) = value {
            let update = PatternUpdate::new(
                name,
                pattern
                    .query_unit()
                    .into_iter()
                    .map(|event| orpheus_pattern::Event {
                        whole: event.whole,
                        part: event.part,
                        value: Box::<str>::from(event.value.sample()),
                    })
                    .collect(),
            );
            self.engine
                .enqueue(EngineCommand::LoadPattern(update))
                .map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    pub(crate) fn binding_summaries(&self) -> Vec<String> {
        self.type_bindings
            .iter()
            .map(|(name, ty)| format!("{name}: {ty}"))
            .collect()
    }
}

fn success_banner(ty: &Type) -> String {
    format!("[{ty}] ok")
}

#[cfg(test)]
mod tests {
    use super::ReplSession;

    #[test]
    fn eval_line_reuses_prior_bindings() {
        let mut session = ReplSession::new();

        assert_eq!(
            session.eval_line("drums = bd sn cp sn"),
            Ok("[Pattern<Sample>] ok".to_owned())
        );
        assert_eq!(
            session.eval_line("copy = drums"),
            Ok("[Pattern<Sample>] ok".to_owned())
        );
    }

    #[test]
    fn sample_patterns_drive_the_embedded_audio_engine() {
        let mut session = ReplSession::new();

        session.eval_line("drums = bd sn cp sn").unwrap();
        let rendered = session
            .engine
            .render_test_block(session.engine.frames_until_boundary_for_test() + 256);

        assert!(rendered.iter().any(|sample| sample.abs() > f32::EPSILON));
    }
}
