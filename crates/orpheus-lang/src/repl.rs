use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

use orpheus_dsp::{EngineCommand, EngineHandle, PatternUpdate, TransportSnapshot};

use crate::eval::eval_into_bindings;
use crate::render_sample_pattern_to_wav;
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
    last_loaded_pattern_name: Option<String>,
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
            last_loaded_pattern_name: None,
        }
    }

    pub(crate) fn eval_line(&mut self, source: &str) -> Result<String, String> {
        if source.starts_with(':') {
            return self.eval_command(source);
        }

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

    fn eval_command(&self, source: &str) -> Result<String, String> {
        let command = source.trim_start_matches(':').trim();
        let Some((name, args)) = command.split_once(char::is_whitespace) else {
            return match command {
                "render" => Err(render_usage().to_owned()),
                "" => Err("empty REPL command".to_owned()),
                other => Err(format!("unknown REPL command `:{other}`")),
            };
        };

        match name {
            "render" => self.render_binding(args),
            other => Err(format!("unknown REPL command `:{other}`")),
        }
    }

    fn render_binding(&self, args: &str) -> Result<String, String> {
        let tokens = args.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 2 {
            return Err(render_usage().to_owned());
        }

        let cycles = if tokens.len() >= 3 {
            tokens
                .last()
                .and_then(|token| token.parse::<u64>().ok())
                .unwrap_or(1)
        } else {
            1
        };
        let path_end = if tokens.len() >= 3 && tokens.last().unwrap().parse::<u64>().is_ok() {
            tokens.len() - 1
        } else {
            tokens.len()
        };

        let binding_name = tokens[0];
        let path = tokens[1..path_end].join(" ");
        if path.is_empty() {
            return Err(render_usage().to_owned());
        }

        let value = self
            .bindings
            .get(binding_name)
            .ok_or_else(|| format!("no binding named `{binding_name}`"))?;
        let Value::SamplePattern(pattern) = value else {
            return Err(format!(
                "binding `{binding_name}` is not a sample pattern and cannot be rendered"
            ));
        };

        render_sample_pattern_to_wav(pattern, &path, cycles).map_err(|error| error.to_string())?;
        Ok(format!(
            "rendered `{binding_name}` to `{path}` ({cycles} cycle(s))"
        ))
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
            self.last_loaded_pattern_name = Some(name.to_owned());
        }

        Ok(())
    }

    pub(crate) fn binding_summaries(&self) -> Vec<String> {
        self.type_bindings
            .iter()
            .map(|(name, ty)| format!("{name}: {ty}"))
            .collect()
    }

    pub(crate) fn last_loaded_pattern_name(&self) -> Option<&str> {
        self.last_loaded_pattern_name.as_deref()
    }

    pub(crate) fn transport_snapshot(&self) -> TransportSnapshot {
        self.engine.transport_snapshot()
    }

    #[cfg(test)]
    pub(crate) fn render_test_block_for_tui(&mut self, frames: u64) -> Vec<f32> {
        self.engine.render_test_block(frames)
    }
}

fn success_banner(ty: &Type) -> String {
    format!("[{ty}] ok")
}

const fn render_usage() -> &'static str {
    "usage: :render <binding> <path> [cycles]"
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::ReplSession;

    fn temp_wav_path() -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("orpheus render {timestamp}.wav"))
    }

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

    #[test]
    fn render_command_exports_a_bound_pattern() {
        let mut session = ReplSession::new();
        let path = temp_wav_path();

        session.eval_line("song = bd sn cp sn").unwrap();
        let message = session
            .eval_line(&format!(":render song {} 2", path.display()))
            .unwrap();

        assert!(message.contains("rendered `song`"));
        assert!(path.exists());
        assert!(fs::metadata(&path).unwrap().len() > 44);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn render_command_rejects_unknown_bindings() {
        let mut session = ReplSession::new();

        let error = session.eval_line(":render nope out.wav 1").unwrap_err();

        assert!(error.contains("no binding named `nope`"));
    }

    #[test]
    fn last_loaded_pattern_name_tracks_sample_bindings() {
        let mut session = ReplSession::new();

        session.eval_line("drums = bd sn cp sn").unwrap();
        assert_eq!(session.last_loaded_pattern_name(), Some("drums"));

        session.eval_line("warp = fast(2)").unwrap();
        assert_eq!(session.last_loaded_pattern_name(), Some("drums"));
    }
}
