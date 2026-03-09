use cpal::{BufferSize, SampleRate, StreamConfig};
use orpheus_pattern::Event;
use rtrb::{Consumer, Producer};
use thiserror::Error;

use crate::command::{EngineCommand, PatternUpdate, new_command_queue};
use crate::scheduler::Scheduler;
use crate::voice::ActiveVoice;

const DEFAULT_SAMPLE_RATE: u32 = 48_000;
const DEFAULT_CHANNELS: u16 = 2;
const DEFAULT_TEMPO_BPM: f32 = 120.0;
const BEATS_PER_CYCLE: f64 = 4.0;
const MAX_ACTIVE_VOICES: usize = 32;

/// Errors raised by the minimal Orpheus audio engine.
#[derive(Debug, Error)]
pub enum EngineError {
    #[error("audio output must have at least one channel")]
    InvalidChannelCount,
    #[error("engine command queue is full")]
    CommandQueueFull,
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
    active_voices: Vec<Option<ActiveVoice>>,
    sample_rate: u32,
    channels: usize,
    current_frame: u64,
    frames_per_cycle: u64,
    current_cycle_start_frame: u64,
    next_cycle_boundary_frame: u64,
    active_pattern: Option<PatternUpdate>,
    pending_pattern: Option<PatternUpdate>,
    prime_initial_pattern: bool,
    last_swap_frame: Option<u64>,
}

impl EngineCore {
    fn new(config: &StreamConfig) -> Result<Self, EngineError> {
        if config.channels == 0 {
            return Err(EngineError::InvalidChannelCount);
        }

        let frames_per_cycle = frames_per_cycle(config.sample_rate.0, DEFAULT_TEMPO_BPM)?;
        Ok(Self {
            scheduler: Scheduler::default(),
            active_voices: vec![None; MAX_ACTIVE_VOICES],
            sample_rate: config.sample_rate.0,
            channels: usize::from(config.channels),
            current_frame: 0,
            frames_per_cycle,
            current_cycle_start_frame: 0,
            next_cycle_boundary_frame: frames_per_cycle,
            active_pattern: None,
            pending_pattern: None,
            prime_initial_pattern: false,
            last_swap_frame: None,
        })
    }

    fn apply_command(&mut self, command: EngineCommand) -> Result<(), EngineError> {
        match command {
            EngineCommand::SwapPattern(pattern_name) => {
                self.pending_pattern = Some(PatternUpdate::silent(pattern_name));
                self.prime_initial_pattern = false;
                Ok(())
            }
            EngineCommand::LoadPattern(pattern) => {
                self.pending_pattern = Some(pattern);
                self.prime_initial_pattern =
                    self.active_pattern.is_none() && self.current_frame == 0;
                Ok(())
            }
            EngineCommand::SetTempo(tempo_bpm) => {
                self.frames_per_cycle = frames_per_cycle(self.sample_rate, tempo_bpm)?;
                if self.current_frame == self.current_cycle_start_frame {
                    self.next_cycle_boundary_frame = self
                        .current_cycle_start_frame
                        .checked_add(self.frames_per_cycle)
                        .ok_or(EngineError::FrameOverflow)?;
                }
                Ok(())
            }
        }
    }

    fn begin_cycle(&mut self) -> Result<(), EngineError> {
        self.current_cycle_start_frame = self.current_frame;
        self.next_cycle_boundary_frame = self
            .current_frame
            .checked_add(self.frames_per_cycle)
            .ok_or(EngineError::FrameOverflow)?;

        if let Some(pattern) = self.pending_pattern.take() {
            self.active_pattern = Some(pattern);
            self.last_swap_frame = Some(self.current_frame);
        }

        if let Some(pattern) = self.active_pattern.as_ref() {
            self.scheduler.schedule_cycle_events(
                self.current_cycle_start_frame,
                self.frames_per_cycle,
                pattern.events().iter().map(|event| Event {
                    whole: event.whole.clone(),
                    part: event.part.clone(),
                    value: event.value.as_ref(),
                }),
            )?;
        }

        Ok(())
    }

    fn render_into_interleaved(&mut self, output: &mut [f32]) -> Result<(), EngineError> {
        if output.len() % self.channels != 0 {
            return Err(EngineError::MisalignedOutputBuffer);
        }

        for frame in output.chunks_exact_mut(self.channels) {
            if self.prime_initial_pattern {
                self.prime_initial_pattern = false;
                self.begin_cycle()?;
            }

            while let Some(trigger) = self.scheduler.pop_due(self.current_frame) {
                self.activate_voice(trigger.voice);
            }

            let mixed = mix_voices(&mut self.active_voices);
            for sample in frame {
                *sample = mixed;
            }

            self.current_frame = self.current_frame.saturating_add(1);
            if self.current_frame == self.next_cycle_boundary_frame {
                self.begin_cycle()?;
            }
        }

        Ok(())
    }

    const fn frames_until_boundary(&self) -> u64 {
        self.next_cycle_boundary_frame
            .saturating_sub(self.current_frame)
    }

    fn activate_voice(&mut self, voice: crate::VoiceKind) {
        if let Some(slot) = self.active_voices.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(ActiveVoice::new(voice, self.sample_rate));
        }
    }
}

/// Render-side engine state intended to live on the audio thread.
#[derive(Debug)]
pub struct RenderEngine {
    command_rx: Consumer<EngineCommand>,
    core: EngineCore,
}

impl RenderEngine {
    fn new(
        command_rx: Consumer<EngineCommand>,
        config: &StreamConfig,
    ) -> Result<Self, EngineError> {
        Ok(Self {
            command_rx,
            core: EngineCore::new(config)?,
        })
    }

    /// Applies any queued commands and renders into an interleaved output buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if command application fails or if `output` does not
    /// contain a whole number of frames for this renderer's channel layout.
    pub fn render_into_interleaved(&mut self, output: &mut [f32]) -> Result<(), EngineError> {
        self.drain_commands()?;
        self.core.render_into_interleaved(output)
    }

    /// Reports whether the most recent swap became active before a cycle boundary.
    ///
    /// # Errors
    ///
    /// Returns an error if queued commands fail while being applied.
    pub fn swap_applied_before_boundary(&mut self) -> Result<bool, EngineError> {
        self.drain_commands()?;
        Ok(self
            .core
            .last_swap_frame
            .is_some_and(|frame| frame % self.core.frames_per_cycle != 0))
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
        self.render_into_interleaved(&mut output)
            .unwrap_or_else(|error| panic!("render test block failed: {error}"));
        output
    }

    /// Returns the active pattern name after the most recently completed cycle.
    #[must_use]
    pub fn active_pattern_name_for_test(&self) -> Option<&str> {
        self.core.active_pattern.as_ref().map(PatternUpdate::name)
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

/// UI-side command producer for the current minimal engine slice.
#[derive(Debug)]
pub struct EngineHandle {
    command_tx: Producer<EngineCommand>,
    test_renderer: Option<RenderEngine>,
}

impl PartialEq for EngineHandle {
    fn eq(&self, other: &Self) -> bool {
        match (self.test_renderer.as_ref(), other.test_renderer.as_ref()) {
            (Some(left), Some(right)) => {
                left.core.sample_rate == right.core.sample_rate
                    && left.core.channels == right.core.channels
                    && left.core.current_frame == right.core.current_frame
                    && left.core.frames_per_cycle == right.core.frames_per_cycle
                    && left.core.current_cycle_start_frame == right.core.current_cycle_start_frame
                    && left.core.next_cycle_boundary_frame == right.core.next_cycle_boundary_frame
                    && left.core.active_pattern == right.core.active_pattern
                    && left.core.pending_pattern == right.core.pending_pattern
                    && left.core.prime_initial_pattern == right.core.prime_initial_pattern
                    && left.core.last_swap_frame == right.core.last_swap_frame
            }
            (None, None) => std::ptr::eq(self, other),
            (None, Some(_)) | (Some(_), None) => false,
        }
    }
}

impl Eq for EngineHandle {}

impl EngineHandle {
    /// Creates a deterministic single-thread test harness without opening an audio device.
    ///
    /// # Panics
    ///
    /// Panics if the built-in stereo test configuration becomes invalid.
    #[must_use]
    pub fn stub() -> Self {
        let config = default_stream_config();
        Self::from_stream_config(&config)
            .unwrap_or_else(|error| panic!("default test stream config must be valid: {error}"))
    }

    /// Creates a single-thread test harness that embeds a renderer internally.
    ///
    /// # Errors
    ///
    /// Returns an error if the stream configuration is invalid for this slice.
    pub fn from_stream_config(config: &StreamConfig) -> Result<Self, EngineError> {
        let (mut handle, renderer) = Self::split_for_stream_config(config)?;
        handle.test_renderer = Some(renderer);
        Ok(handle)
    }

    /// Splits the UI command handle from the render-side engine.
    ///
    /// # Errors
    ///
    /// Returns an error if the stream configuration is invalid for this slice.
    pub fn split_for_stream_config(
        config: &StreamConfig,
    ) -> Result<(Self, RenderEngine), EngineError> {
        let (command_tx, command_rx) = new_command_queue();
        let renderer = RenderEngine::new(command_rx, config)?;
        Ok((
            Self {
                command_tx,
                test_renderer: None,
            },
            renderer,
        ))
    }

    /// Splits a deterministic stereo handle and renderer pair for tests.
    ///
    /// # Panics
    ///
    /// Panics if the built-in stereo test configuration becomes invalid.
    #[must_use]
    pub fn split_for_test() -> (Self, RenderEngine) {
        let config = default_stream_config();
        Self::split_for_stream_config(&config)
            .unwrap_or_else(|error| panic!("default test stream config must be valid: {error}"))
    }

    /// Enqueues a command for the render thread to observe on the next render call.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::CommandQueueFull`] when the bounded command queue
    /// has no remaining capacity.
    pub fn enqueue(&mut self, command: EngineCommand) -> Result<(), EngineError> {
        self.command_tx
            .push(command)
            .map_err(|_| EngineError::CommandQueueFull)
    }

    /// Reports whether a deferred pattern swap leaked through before the next
    /// cycle boundary in the embedded test renderer.
    ///
    /// # Panics
    ///
    /// Panics if the handle does not own an embedded test renderer or if
    /// queued commands fail while being applied.
    pub fn swap_applied_before_boundary(&mut self) -> bool {
        self.test_renderer_mut()
            .swap_applied_before_boundary()
            .unwrap_or_else(|error| panic!("swap check failed while draining commands: {error}"))
    }

    /// Schedules a built-in test trigger in the embedded renderer.
    ///
    /// # Panics
    ///
    /// Panics if the handle does not own an embedded test renderer.
    pub fn schedule_test_trigger(&mut self, frame: u64, token: &str) {
        self.test_renderer_mut().schedule_test_trigger(frame, token);
    }

    /// Renders `frames` stereo frames through the embedded test renderer.
    ///
    /// # Panics
    ///
    /// Panics if the handle does not own an embedded test renderer.
    #[must_use]
    pub fn render_test_block(&mut self, frames: u64) -> Vec<f32> {
        self.test_renderer_mut().render_test_block(frames)
    }

    /// Returns the active pattern name for the embedded test renderer.
    ///
    /// # Panics
    ///
    /// Panics if the handle does not own an embedded test renderer.
    #[must_use]
    pub fn active_pattern_name_for_test(&self) -> Option<&str> {
        self.test_renderer_ref().active_pattern_name_for_test()
    }

    /// Returns how many frames remain before the next boundary in the embedded
    /// test renderer.
    ///
    /// # Panics
    ///
    /// Panics if the handle does not own an embedded test renderer.
    #[must_use]
    pub fn frames_until_boundary_for_test(&self) -> u64 {
        self.test_renderer_ref().frames_until_boundary_for_test()
    }

    /// Returns the current cycle length in frames for the embedded test renderer.
    ///
    /// # Panics
    ///
    /// Panics if the handle does not own an embedded test renderer.
    #[must_use]
    pub fn frames_per_cycle_for_test(&self) -> u64 {
        self.test_renderer_ref().frames_per_cycle_for_test()
    }

    fn test_renderer_mut(&mut self) -> &mut RenderEngine {
        self.test_renderer
            .as_mut()
            .unwrap_or_else(|| panic!("test-only renderer access requires EngineHandle::stub()"))
    }

    fn test_renderer_ref(&self) -> &RenderEngine {
        self.test_renderer
            .as_ref()
            .unwrap_or_else(|| panic!("test-only renderer access requires EngineHandle::stub()"))
    }
}

const fn default_stream_config() -> StreamConfig {
    StreamConfig {
        channels: DEFAULT_CHANNELS,
        sample_rate: SampleRate(DEFAULT_SAMPLE_RATE),
        buffer_size: BufferSize::Default,
    }
}

fn frames_per_cycle(sample_rate: u32, tempo_bpm: f32) -> Result<u64, EngineError> {
    use std::time::Duration;

    if !tempo_bpm.is_finite() || tempo_bpm <= 0.0 {
        return Err(EngineError::InvalidTempo);
    }

    let cycle_seconds = (60.0 * BEATS_PER_CYCLE) / f64::from(tempo_bpm);
    if !cycle_seconds.is_finite() || cycle_seconds > Duration::MAX.as_secs_f64() {
        return Err(EngineError::FrameOverflow);
    }

    let cycle_duration = Duration::from_secs_f64(cycle_seconds);
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

fn mix_voices(active_voices: &mut [Option<ActiveVoice>]) -> f32 {
    let mut mixed = 0.0_f32;
    for slot in active_voices {
        if let Some(voice) = slot.as_mut() {
            if let Some(sample) = voice.next_sample() {
                mixed += sample;
            } else {
                *slot = None;
            }
        }
    }
    mixed.clamp(-1.0, 1.0)
}
