use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use orpheus_dsp::{
    EngineCommand, EngineHandle, PatternUpdate, SampleBank, SampleTrigger, TransportSnapshot,
    load_sample_bank_from_directory,
};

use crate::eval::{eval_into_bindings, render_sample_pattern_to_file_with_bank};
use crate::loader::load_file_runtime_strict;
use crate::types::infer_into_bindings;
use crate::{ReplMode, Type, Value};

pub(crate) struct Session {
    mode: ReplMode,
    engine: EngineHandle,
    sample_bank: SampleBank,
    sample_directory: Option<PathBuf>,
    bindings: BTreeMap<String, Value>,
    type_bindings: BTreeMap<String, Type>,
    pattern_display: RefCell<PatternDisplayState>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PatternDisplayState {
    active_pattern_name: Option<String>,
    pending_pattern_name: Option<String>,
    pending_enqueued_after_publish: Option<u64>,
    last_loaded_pattern_name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TransportView {
    snapshot: TransportSnapshot,
    active_pattern_name: Option<String>,
    pending_pattern_name: Option<String>,
}

impl TransportView {
    #[must_use]
    pub const fn snapshot(&self) -> &TransportSnapshot {
        &self.snapshot
    }

    #[must_use]
    pub fn active_pattern_name(&self) -> Option<&str> {
        self.active_pattern_name.as_deref()
    }

    #[must_use]
    pub fn pending_pattern_name(&self) -> Option<&str> {
        self.pending_pattern_name.as_deref()
    }
}

impl Session {
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::with_engine(EngineHandle::stub())
    }

    pub(crate) fn with_engine(engine: EngineHandle) -> Self {
        Self {
            mode: ReplMode::Loose,
            engine,
            sample_bank: SampleBank::load_builtin(),
            sample_directory: None,
            bindings: BTreeMap::new(),
            type_bindings: BTreeMap::new(),
            pattern_display: RefCell::new(PatternDisplayState::default()),
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
        Ok(success_banner(&name, &ty))
    }

    fn eval_command(&mut self, source: &str) -> Result<String, String> {
        let command = source.trim_start_matches(':').trim();
        if command.is_empty() {
            return Err("empty REPL command".to_owned());
        }
        let (name, args) = command
            .split_once(char::is_whitespace)
            .map_or((command, ""), |(name, args)| (name, args.trim()));

        match name {
            "render" => {
                if args.is_empty() {
                    Err(render_usage().to_owned())
                } else {
                    self.render_binding(args)
                }
            }
            "export" => {
                if args.is_empty() {
                    Err(export_usage().to_owned())
                } else {
                    self.export_binding(args)
                }
            }
            "tempo" => {
                if args.is_empty() {
                    Err(tempo_usage().to_owned())
                } else {
                    self.set_tempo(args)
                }
            }
            "samples" => {
                if args.is_empty() {
                    Err(samples_usage().to_owned())
                } else {
                    self.load_sample_directory(args)
                }
            }
            "open" => {
                if args.is_empty() {
                    Err(open_usage().to_owned())
                } else {
                    self.open_file(args)
                }
            }
            "reload-samples" => self.reload_sample_directory(args),
            "play" => self.play_transport(args),
            "stop" => self.stop_transport(args),
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

        render_sample_pattern_to_file_with_bank(pattern, &path, cycles, &self.sample_bank)
            .map_err(|error| error.to_string())?;
        Ok(format!(
            "rendered `{binding_name}` to `{path}` ({cycles} cycle(s))"
        ))
    }

    fn export_binding(&self, args: &str) -> Result<String, String> {
        let tokens = args.split_whitespace().collect::<Vec<_>>();
        if tokens.len() < 2 {
            return Err(export_usage().to_owned());
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
            return Err(export_usage().to_owned());
        }

        let value = self
            .bindings
            .get(binding_name)
            .ok_or_else(|| format!("no binding named `{binding_name}`"))?;

        match value {
            Value::SamplePattern(pattern) => {
                crate::eval::export_sample_pattern_to_csv(pattern, &path, cycles)
                    .map_err(|error| error.to_string())?;
            }
            Value::NumberPattern(pattern) => {
                crate::eval::export_number_pattern_to_csv(pattern, &path, cycles)
                    .map_err(|error| error.to_string())?;
            }
            Value::Function(_) | Value::String(_) => {
                return Err(format!(
                    "binding `{binding_name}` is a {} and cannot be exported",
                    value.kind_name()
                ));
            }
        }

        Ok(format!(
            "exported `{binding_name}` to `{path}` ({cycles} cycle(s))"
        ))
    }

    fn set_tempo(&mut self, args: &str) -> Result<String, String> {
        let tokens = args.split_whitespace().collect::<Vec<_>>();
        if tokens.len() != 1 {
            return Err(tempo_usage().to_owned());
        }

        let tempo_bpm = tokens[0]
            .parse::<f32>()
            .map_err(|_| "tempo must be a finite positive BPM".to_owned())?;
        if !tempo_bpm.is_finite() || tempo_bpm <= 0.0 {
            return Err("tempo must be a finite positive BPM".to_owned());
        }

        self.engine
            .enqueue(EngineCommand::SetTempo(tempo_bpm))
            .map_err(|error| error.to_string())?;
        Ok(format!("tempo set to {tempo_bpm} BPM"))
    }

    fn load_sample_directory(&mut self, args: &str) -> Result<String, String> {
        let directory = PathBuf::from(args);
        let sample_bank =
            load_sample_bank_from_directory(&directory).map_err(|error| error.to_string())?;
        let available_tokens = sample_bank.available_tokens();
        self.sample_bank = sample_bank.clone();
        self.sample_directory = Some(directory.clone());
        self.engine
            .enqueue(EngineCommand::ReplaceSampleBank(sample_bank))
            .map_err(|error| error.to_string())?;
        Ok(format!(
            "loaded sample overrides from `{}` ({})",
            directory.display(),
            available_tokens.join(", ")
        ))
    }

    pub(crate) fn open_file(&mut self, path: impl AsRef<Path>) -> Result<String, String> {
        let path = path.as_ref();
        let loaded = load_file_runtime_strict(path).map_err(|error| error.to_string())?;
        let binding_names = loaded
            .type_bindings
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        let last_binding_name = loaded.last_binding_name.clone();

        self.bindings = loaded.value_bindings;
        self.type_bindings = loaded.type_bindings;
        *self.pattern_display.borrow_mut() = PatternDisplayState::default();

        if let Some(name) = last_binding_name {
            if let Some(value) = self.bindings.get(&name).cloned() {
                self.push_pattern_update(&name, &value)?;
            }
        }

        Ok(format!("opened `{}` ({binding_names})", path.display()))
    }

    fn reload_sample_directory(&mut self, args: &str) -> Result<String, String> {
        if !args.is_empty() {
            return Err(reload_samples_usage().to_owned());
        }

        let Some(directory) = self.sample_directory.clone() else {
            return Err("no sample directory has been configured".to_owned());
        };
        let sample_bank =
            load_sample_bank_from_directory(&directory).map_err(|error| error.to_string())?;
        let available_tokens = sample_bank.available_tokens();
        self.sample_bank = sample_bank.clone();
        self.engine
            .enqueue(EngineCommand::ReplaceSampleBank(sample_bank))
            .map_err(|error| error.to_string())?;
        Ok(format!(
            "reloaded sample overrides from `{}` ({})",
            directory.display(),
            available_tokens.join(", ")
        ))
    }

    fn play_transport(&mut self, args: &str) -> Result<String, String> {
        if !args.is_empty() {
            return Err(play_usage().to_owned());
        }

        self.engine
            .enqueue(EngineCommand::PlayTransport)
            .map_err(|error| format!("failed to enqueue play transport command: {error}"))?;
        Ok("transport playing".to_owned())
    }

    fn stop_transport(&mut self, args: &str) -> Result<String, String> {
        if !args.is_empty() {
            return Err(stop_usage().to_owned());
        }

        self.engine
            .enqueue(EngineCommand::StopTransport)
            .map_err(|error| format!("failed to enqueue stop transport command: {error}"))?;
        Ok("transport stopped".to_owned())
    }

    fn push_pattern_update(&mut self, name: &str, value: &Value) -> Result<(), String> {
        if let Value::SamplePattern(pattern) = value {
            let enqueue_publish = self.engine.transport_snapshot().publish_epoch();
            let events = pattern.query_unit();
            let update = PatternUpdate::new(
                name,
                events
                    .into_iter()
                    .map(|event| orpheus_pattern::Event {
                        whole: event.whole,
                        part: event.part,
                        value: {
                            let mut trigger = SampleTrigger::named(event.value.sample())
                                .with_gain(event.value.gain())
                                .with_pan(event.value.pan())
                                .with_rate(event.value.rate())
                                .with_slice(event.value.slice_start(), event.value.slice_end());
                            if let Some(cutoff_hz) = event.value.hpf_cutoff_hz() {
                                trigger = trigger.with_hpf_cutoff_hz(cutoff_hz);
                            }
                            if let Some(cutoff_hz) = event.value.lpf_cutoff_hz() {
                                trigger = trigger.with_lpf_cutoff_hz(cutoff_hz);
                            }
                            trigger
                        },
                    })
                    .collect(),
            );
            self.engine
                .enqueue(EngineCommand::LoadPattern(update))
                .map_err(|error| {
                    format!("failed to enqueue load pattern command for `{name}`: {error}")
                })?;
            let mut display = self.pattern_display.borrow_mut();
            if display.active_pattern_name.is_none() && enqueue_publish != 0 {
                if let Some(last_loaded_pattern_name) = display.last_loaded_pattern_name.clone() {
                    display.active_pattern_name = Some(last_loaded_pattern_name);
                }
            }
            display.last_loaded_pattern_name = Some(name.to_owned());
            display.pending_pattern_name = Some(name.to_owned());
            display.pending_enqueued_after_publish = Some(enqueue_publish);
        }

        Ok(())
    }

    pub(crate) fn binding_summaries(&self) -> Vec<String> {
        self.type_bindings
            .iter()
            .map(|(name, ty)| format!("{name}: {ty}"))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn last_loaded_pattern_name(&self) -> Option<String> {
        self.pattern_display
            .borrow()
            .last_loaded_pattern_name
            .clone()
    }

    pub(crate) fn transport_snapshot(&self) -> TransportSnapshot {
        self.transport_view().snapshot
    }

    pub(crate) fn transport_view(&self) -> TransportView {
        let snapshot = self.engine.transport_snapshot();
        let mut display = self.pattern_display.borrow_mut();
        if let Some(pending_name) = display.pending_pattern_name.clone() {
            let enqueued_after_publish = display
                .pending_enqueued_after_publish
                .unwrap_or_else(|| snapshot.publish_epoch());
            if !snapshot.has_pending_pattern() && snapshot.publish_epoch() != enqueued_after_publish
            {
                display.active_pattern_name = Some(pending_name);
                display.pending_pattern_name = None;
                display.pending_enqueued_after_publish = None;
            }
        } else if display.active_pattern_name.is_none() && snapshot.current_frame() != 0 {
            if let Some(last_loaded_pattern_name) = display.last_loaded_pattern_name.clone() {
                display.active_pattern_name = Some(last_loaded_pattern_name);
            }
        }

        TransportView {
            snapshot,
            active_pattern_name: display.active_pattern_name.clone(),
            pending_pattern_name: display.pending_pattern_name.clone(),
        }
    }

    #[cfg(test)]
    pub(crate) fn render_test_block_for_tui(&mut self, frames: u64) -> Vec<f32> {
        self.engine.render_test_block(frames)
    }

    #[cfg(test)]
    pub(crate) fn frames_until_boundary_for_tui(&self) -> u64 {
        self.engine.frames_until_boundary_for_test()
    }
}

fn success_banner(name: &str, ty: &Type) -> String {
    format!("✓ bound {name}: {ty}")
}

const fn render_usage() -> &'static str {
    "usage: :render <binding> <path> [cycles]"
}

const fn export_usage() -> &'static str {
    "usage: :export <binding> <path> [cycles]"
}

const fn tempo_usage() -> &'static str {
    "usage: :tempo <bpm>"
}

const fn samples_usage() -> &'static str {
    "usage: :samples <directory>"
}

const fn open_usage() -> &'static str {
    "usage: :open <path>"
}

const fn reload_samples_usage() -> &'static str {
    "usage: :reload-samples"
}

const fn play_usage() -> &'static str {
    "usage: :play"
}

const fn stop_usage() -> &'static str {
    "usage: :stop"
}

impl Session {
    #[cfg(test)]
    pub(crate) const fn engine(&mut self) -> &mut EngineHandle {
        &mut self.engine
    }
}
