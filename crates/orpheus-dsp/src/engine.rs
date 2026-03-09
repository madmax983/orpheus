use cpal::{BufferSize, SampleRate, StreamConfig};
use rtrb::{Consumer, Producer};
use thiserror::Error;

use crate::command::{EngineCommand, new_command_queue};
use crate::scheduler::Scheduler;
use crate::voice::ActiveVoice;

const DEFAULT_SAMPLE_RATE: u32 = 48_000;
const DEFAULT_CHANNELS: u16 = 2;
const DEFAULT_TEMPO_BPM: f32 = 120.0;
const BEATS_PER_CYCLE: f64 = 4.0;

/// Errors raised by the minimal Orpheus audio engine.
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("audio output must have at least one channel")]
    InvalidChannelCount,
    #[error("tempo must be a finite positive value")]
    InvalidTempo,
    #[error("pattern time produced a negative cycle offset")]
    NegativeCycleOffset,
    #[error("sample-clock conversion overflowed the supported range")]
    FrameOverflow,
    #[error("unknown built-in voice token `{0}`")]
    UnknownVoice(String),
    #[error("output buffer length must be a whole number of frames")]
    MisalignedOutputBuffer,
}

#[derive(Debug)]
struct EngineCore {
    scheduler: Scheduler,
    active_voices: Vec<ActiveVoice>,
    sample_rate: u32,
    channels: usize,
    current_frame: u64,
    frames_per_cycle: u64,
    active_pattern_name: Option<String>,
    pending_pattern_name: Option<String>,
    swap_applied_before_boundary: bool,
}

impl EngineCore {
    fn new(config: &StreamConfig) -> Result<Self, EngineError> {
        if config.channels == 0 {
            return Err(EngineError::InvalidChannelCount);
        }

        let frames_per_cycle = frames_per_cycle(config.sample_rate.0, DEFAULT_TEMPO_BPM)?;
        Ok(Self {
            scheduler: Scheduler::default(),
            active_voices: Vec::new(),
            sample_rate: config.sample_rate.0,
            channels: usize::from(config.channels),
            current_frame: 0,
            frames_per_cycle,
            active_pattern_name: None,
            pending_pattern_name: None,
            swap_applied_before_boundary: false,
        })
    }

    fn apply_command(&mut self, command: EngineCommand) -> Result<(), EngineError> {
        match command {
            EngineCommand::SwapPattern(pattern_name) => {
                self.pending_pattern_name = Some(pattern_name);
                Ok(())
            }
            EngineCommand::SetTempo(tempo_bpm) => {
                self.frames_per_cycle = frames_per_cycle(self.sample_rate, tempo_bpm)?;
                Ok(())
            }
        }
    }

    fn render_into_interleaved(&mut self, output: &mut [f32]) -> Result<(), EngineError> {
        if output.len() % self.channels != 0 {
            return Err(EngineError::MisalignedOutputBuffer);
        }

        for frame in output.chunks_exact_mut(self.channels) {
            let due = self.scheduler.drain_due(self.current_frame);
            for trigger in due {
                self.active_voices
                    .push(ActiveVoice::new(trigger.voice, self.sample_rate));
            }

            let mixed = mix_voices(&mut self.active_voices);
            for sample in frame {
                *sample = mixed;
            }

            self.current_frame = self.current_frame.saturating_add(1);
            if self.current_frame % self.frames_per_cycle == 0 {
                if let Some(pattern_name) = self.pending_pattern_name.take() {
                    self.active_pattern_name = Some(pattern_name);
                }
            }
        }

        Ok(())
    }

    const fn frames_until_boundary(&self) -> u64 {
        let progress = self.current_frame % self.frames_per_cycle;
        if progress == 0 {
            self.frames_per_cycle
        } else {
            self.frames_per_cycle - progress
        }
    }
}

/// Command producer plus test-friendly render loop for the current vertical slice.
#[derive(Debug)]
pub struct EngineHandle {
    command_tx: Producer<EngineCommand>,
    command_rx: Consumer<EngineCommand>,
    core: EngineCore,
}

impl EngineHandle {
    /// Creates a deterministic stereo engine without opening an audio device.
    ///
    /// # Panics
    ///
    /// Panics if the built-in stereo test configuration becomes invalid.
    #[must_use]
    pub fn stub() -> Self {
        let config = StreamConfig {
            channels: DEFAULT_CHANNELS,
            sample_rate: SampleRate(DEFAULT_SAMPLE_RATE),
            buffer_size: BufferSize::Default,
        };
        Self::from_stream_config(&config)
            .unwrap_or_else(|error| panic!("default test stream config must be valid: {error}"))
    }

    /// Creates an engine core that matches a `cpal` output configuration.
    ///
    /// This keeps the render path aligned with the eventual real-time audio
    /// thread without forcing tests to open a device.
    ///
    /// # Errors
    ///
    /// Returns an error if the stream configuration is invalid for this slice.
    pub fn from_stream_config(config: &StreamConfig) -> Result<Self, EngineError> {
        let (command_tx, command_rx) = new_command_queue();
        let core = EngineCore::new(config)?;
        Ok(Self {
            command_tx,
            command_rx,
            core,
        })
    }

    /// Enqueues a command for the audio thread to observe on the next render call.
    ///
    /// # Panics
    ///
    /// Panics if the bounded command queue is exhausted.
    pub fn enqueue(&mut self, command: EngineCommand) {
        self.command_tx
            .push(command)
            .unwrap_or_else(|_| panic!("engine command queue overflowed"));
    }

    /// Reports whether a deferred pattern swap leaked through before the next
    /// cycle boundary.
    ///
    /// # Panics
    ///
    /// Panics if queued commands fail while being applied in the test loop.
    pub fn swap_applied_before_boundary(&mut self) -> bool {
        self.drain_commands()
            .unwrap_or_else(|error| panic!("swap check failed while draining commands: {error}"));
        self.core.swap_applied_before_boundary
    }

    /// Schedules a built-in test trigger at an absolute sample frame.
    ///
    /// # Panics
    ///
    /// Panics if the named token has no built-in synthesized fallback.
    pub fn schedule_test_trigger(&mut self, frame: u64, token: &str) {
        self.core.scheduler.push_test_event(frame, token);
    }

    /// Renders `frames` stereo frames into an owned interleaved buffer.
    ///
    /// # Panics
    ///
    /// Panics if command application or buffer rendering fails.
    #[must_use]
    pub fn render_test_block(&mut self, frames: u64) -> Vec<f32> {
        let frames_usize = usize::try_from(frames)
            .unwrap_or_else(|_| panic!("requested render block does not fit in memory"));
        let mut output = vec![0.0_f32; frames_usize * self.core.channels];
        self.drain_commands()
            .and_then(|()| self.core.render_into_interleaved(&mut output))
            .unwrap_or_else(|error| panic!("render test block failed: {error}"));
        output
    }

    /// Returns the active pattern name after the most recently completed cycle.
    #[must_use]
    pub fn active_pattern_name_for_test(&self) -> Option<&str> {
        self.core.active_pattern_name.as_deref()
    }

    /// Returns how many frames remain before the next cycle boundary.
    #[must_use]
    pub const fn frames_until_boundary_for_test(&self) -> u64 {
        self.core.frames_until_boundary()
    }

    /// Returns the current cycle length in frames for test assertions.
    #[must_use]
    pub const fn frames_per_cycle_for_test(&self) -> u64 {
        self.core.frames_per_cycle
    }

    fn drain_commands(&mut self) -> Result<(), EngineError> {
        while let Ok(command) = self.command_rx.pop() {
            self.core.apply_command(command)?;
        }
        Ok(())
    }
}

fn frames_per_cycle(sample_rate: u32, tempo_bpm: f32) -> Result<u64, EngineError> {
    use std::time::Duration;

    if !tempo_bpm.is_finite() || tempo_bpm <= 0.0 {
        return Err(EngineError::InvalidTempo);
    }

    let cycle_duration = Duration::from_secs_f64((60.0 * BEATS_PER_CYCLE) / f64::from(tempo_bpm));
    let frames = cycle_duration
        .as_nanos()
        .checked_mul(u128::from(sample_rate))
        .ok_or(EngineError::FrameOverflow)?
        / 1_000_000_000;
    let frames = u64::try_from(frames).map_err(|_| EngineError::FrameOverflow)?;
    if frames == 0 {
        return Err(EngineError::FrameOverflow);
    }

    Ok(frames)
}

fn mix_voices(active_voices: &mut Vec<ActiveVoice>) -> f32 {
    let mut mixed = 0.0_f32;
    active_voices.retain_mut(|voice| {
        voice.next_sample().is_some_and(|sample| {
            mixed += sample;
            true
        })
    });
    mixed.clamp(-1.0, 1.0)
}
