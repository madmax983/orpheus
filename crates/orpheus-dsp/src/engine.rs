//! The `engine` module encapsulates the real-time audio rendering system.
//!
//! This module coordinates the interaction between system audio APIs (like ALSA or `CoreAudio`),
//! lock-free command queues, and the internal voice scheduler. It provides a thread-safe
//! `EngineHandle` for the front-end to control the `RenderEngine` running on the audio thread.

use cpal::{BufferSize, SampleRate, StreamConfig};
use orpheus_pattern::Event;
use rtrb::{Consumer, Producer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use thiserror::Error;

use crate::command::{EngineCommand, PatternUpdate, new_command_queue};
use crate::effects::BusEffectState;
use crate::routing::{BusEffectSpec, RoutingSnapshot, TrackSource};
use crate::sample_bank::SampleBank;
use crate::scheduler::Scheduler;
use crate::voice::ActiveVoice;

pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;
const DEFAULT_CHANNELS: u16 = 2;
pub const DEFAULT_TEMPO_BPM: f32 = 120.0;
const BEATS_PER_CYCLE: f64 = 4.0;
const MAX_ACTIVE_VOICES: usize = 32;

/// A UI-readable snapshot of the transport clock.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportSnapshot {
    publish_epoch: u64,
    current_frame: u64,
    current_cycle_start_frame: u64,
    frames_per_cycle: u64,
    tempo_bpm_bits: u32,
    is_playing: bool,
    has_pending_pattern: bool,
    has_pending_routing: bool,
}

impl TransportSnapshot {
    #[must_use]
    pub const fn publish_epoch(&self) -> u64 {
        self.publish_epoch
    }

    #[must_use]
    pub const fn current_frame(&self) -> u64 {
        self.current_frame
    }

    #[must_use]
    pub const fn current_cycle_start_frame(&self) -> u64 {
        self.current_cycle_start_frame
    }

    #[must_use]
    pub const fn frames_per_cycle(&self) -> u64 {
        self.frames_per_cycle
    }

    #[must_use]
    pub const fn tempo_bpm(&self) -> f32 {
        f32::from_bits(self.tempo_bpm_bits)
    }

    #[must_use]
    pub const fn is_playing(&self) -> bool {
        self.is_playing
    }

    #[must_use]
    pub const fn has_pending_pattern(&self) -> bool {
        self.has_pending_pattern
    }

    #[must_use]
    pub const fn has_pending_routing(&self) -> bool {
        self.has_pending_routing
    }
}

#[derive(Debug, Default)]
struct SharedTransport {
    publish_epoch: AtomicU64,
    current_frame: AtomicU64,
    current_cycle_start_frame: AtomicU64,
    frames_per_cycle: AtomicU64,
    tempo_bpm_bits: AtomicU32,
    is_playing: AtomicBool,
    has_pending_pattern: AtomicBool,
    has_pending_routing: AtomicBool,
}

impl SharedTransport {
    fn publish(&self, core: &EngineCore) {
        // Start the write transaction. Relaxed is sufficient because the
        // atomic fence handles the required release semantics.
        self.publish_epoch.fetch_add(1, Ordering::Relaxed);
        std::sync::atomic::fence(Ordering::Release);

        self.current_frame
            .store(core.current_frame, Ordering::Relaxed);
        self.current_cycle_start_frame
            .store(core.current_cycle_start_frame, Ordering::Relaxed);
        self.frames_per_cycle
            .store(core.frames_per_cycle, Ordering::Relaxed);
        self.tempo_bpm_bits
            .store(core.tempo_bpm.to_bits(), Ordering::Relaxed);
        self.is_playing.store(core.is_playing, Ordering::Relaxed);
        self.has_pending_pattern
            .store(core.pending_pattern_name.is_some(), Ordering::Relaxed);
        self.has_pending_routing
            .store(core.pending_routing.is_some(), Ordering::Relaxed);

        // Commit the write transaction.
        std::sync::atomic::fence(Ordering::Release);
        self.publish_epoch.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> TransportSnapshot {
        loop {
            let start_epoch = self.publish_epoch.load(Ordering::Relaxed);
            std::sync::atomic::fence(Ordering::Acquire);

            // If odd, a write is in progress. Wait for it to finish.
            if start_epoch % 2 != 0 {
                std::hint::spin_loop();
                continue;
            }

            let snap = TransportSnapshot {
                publish_epoch: start_epoch,
                current_frame: self.current_frame.load(Ordering::Relaxed),
                current_cycle_start_frame: self.current_cycle_start_frame.load(Ordering::Relaxed),
                frames_per_cycle: self.frames_per_cycle.load(Ordering::Relaxed),
                tempo_bpm_bits: self.tempo_bpm_bits.load(Ordering::Relaxed),
                is_playing: self.is_playing.load(Ordering::Relaxed),
                has_pending_pattern: self.has_pending_pattern.load(Ordering::Relaxed),
                has_pending_routing: self.has_pending_routing.load(Ordering::Relaxed),
            };

            std::sync::atomic::fence(Ordering::Acquire);
            let end_epoch = self.publish_epoch.load(Ordering::Relaxed);
            // If the epoch is unchanged, we observed a consistent state.
            if start_epoch == end_epoch {
                return snap;
            }
        }
    }
}

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
    sample_bank: SampleBank,
    active_routing: RoutingSnapshot,
    pending_routing: Option<RoutingSnapshot>,
    bus_effect_states: Vec<Option<BusEffectState>>,
    sample_rate: u32,
    channels: usize,
    current_frame: u64,
    tempo_bpm: f32,
    is_playing: bool,
    frames_per_cycle: u64,
    current_cycle_start_frame: u64,
    next_cycle_boundary_frame: u64,
    active_pattern_name: Option<Box<str>>,
    pending_pattern_name: Option<Box<str>>,
    pending_sample_bank: Option<SampleBank>,
    prime_initial_routing: bool,
    last_swap_frame: Option<u64>,
    track_mix_buffer: Vec<(f32, f32)>,
    bus_mix_buffer: Vec<(f32, f32)>,
}

impl EngineCore {
    fn new(config: &StreamConfig) -> Result<Self, EngineError> {
        if config.channels == 0 {
            return Err(EngineError::InvalidChannelCount);
        }

        let frames_per_cycle = frames_per_cycle(config.sample_rate.0, DEFAULT_TEMPO_BPM)?;
        let active_routing = default_main_routing_snapshot();
        let track_mix_buffer = vec![(0.0, 0.0); active_routing.tracks().len()];
        let bus_mix_buffer = vec![(0.0, 0.0); active_routing.buses().len()];
        Ok(Self {
            scheduler: Scheduler::default(),
            active_voices: std::iter::repeat_with(|| None)
                .take(MAX_ACTIVE_VOICES)
                .collect(),
            sample_bank: SampleBank::load_builtin(),
            active_routing,
            pending_routing: None,
            bus_effect_states: Vec::new(),
            sample_rate: config.sample_rate.0,
            channels: usize::from(config.channels),
            current_frame: 0,
            tempo_bpm: DEFAULT_TEMPO_BPM,
            is_playing: true,
            frames_per_cycle,
            current_cycle_start_frame: 0,
            next_cycle_boundary_frame: frames_per_cycle,
            active_pattern_name: None,
            pending_pattern_name: None,
            pending_sample_bank: None,
            prime_initial_routing: false,
            last_swap_frame: None,
            track_mix_buffer,
            bus_mix_buffer,
        })
    }

    fn apply_command(&mut self, command: EngineCommand) -> Result<(), EngineError> {
        match command {
            EngineCommand::SwapPattern(pattern_name) => {
                self.pending_routing = Some(compatibility_routing_snapshot(
                    &PatternUpdate::silent(pattern_name.clone()),
                ));
                self.pending_pattern_name = Some(pattern_name.into_boxed_str());
                self.prime_initial_routing = false;
                Ok(())
            }
            EngineCommand::LoadPattern(pattern) => {
                self.pending_routing = Some(compatibility_routing_snapshot(&pattern));
                self.pending_pattern_name = Some(pattern.name().into());
                self.prime_initial_routing =
                    self.active_pattern_name.is_none() && self.current_frame == 0;
                Ok(())
            }
            EngineCommand::SwapRoutingSnapshot(snapshot) => {
                self.pending_routing = Some(snapshot);
                self.pending_pattern_name = None;
                self.prime_initial_routing = false;
                Ok(())
            }
            EngineCommand::ReplaceSampleBank(sample_bank) => {
                self.pending_sample_bank = Some(sample_bank);
                Ok(())
            }
            EngineCommand::SetTempo(tempo_bpm) => {
                let frames_per_cycle = frames_per_cycle(self.sample_rate, tempo_bpm)?;
                self.tempo_bpm = tempo_bpm;
                self.frames_per_cycle = frames_per_cycle;
                if self.current_frame == self.current_cycle_start_frame {
                    self.next_cycle_boundary_frame = self
                        .current_cycle_start_frame
                        .checked_add(self.frames_per_cycle)
                        .ok_or(EngineError::FrameOverflow)?;
                }
                Ok(())
            }
            EngineCommand::PlayTransport => {
                self.play_transport();
                Ok(())
            }
            EngineCommand::StopTransport => {
                self.stop_transport();
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

        if let Some(sample_bank) = self.pending_sample_bank.take() {
            self.sample_bank = sample_bank;
        }

        if let Some(routing) = self.pending_routing.take() {
            self.adopt_routing_snapshot(routing)?;
            self.active_pattern_name = self.pending_pattern_name.take();
            self.last_swap_frame = Some(self.current_frame);
        }

        self.sync_bus_effect_timing()?;

        for track in self.active_routing.tracks() {
            if let TrackSource::SamplePattern(events) = track.source() {
                self.scheduler.schedule_cycle_events(
                    track.id(),
                    self.current_cycle_start_frame,
                    self.frames_per_cycle,
                    events.iter().map(|event| Event {
                        whole: event.whole.clone(),
                        part: event.part.clone(),
                        value: &event.value,
                    }),
                )?;
            }
        }

        Ok(())
    }

    fn render_into_interleaved(&mut self, output: &mut [f32]) -> Result<(), EngineError> {
        if output.len() % self.channels != 0 {
            return Err(EngineError::MisalignedOutputBuffer);
        }

        for frame in output.chunks_exact_mut(self.channels) {
            if !self.is_playing {
                frame.fill(0.0);
                continue;
            }

            if self.prime_initial_routing {
                self.prime_initial_routing = false;
                self.begin_cycle()?;
            }

            while let Some(trigger) = self.scheduler.pop_due(self.current_frame) {
                self.activate_trigger(&trigger);
            }

            let (left, right) = self.mix_routed_voices();
            if let Some(first) = frame.first_mut() {
                *first = left;
            }
            if frame.len() >= 2 {
                frame[1] = right;
            }
            let mono_fill = (left + right) * 0.5;
            for sample in frame.iter_mut().skip(2) {
                *sample = mono_fill;
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

    fn activate_trigger(&mut self, trigger: &crate::scheduler::ScheduledTrigger) {
        if let Some(slot) = self.active_voices.iter_mut().find(|slot| slot.is_none()) {
            *slot = self
                .sample_bank
                .resolve_trigger(&trigger.trigger)
                .map(|(sample, resolved_trigger)| {
                    ActiveVoice::from_sample(
                        trigger.track_id,
                        sample,
                        self.sample_rate,
                        self.frames_per_cycle,
                        &resolved_trigger,
                    )
                })
                .or_else(|| {
                    trigger.fallback_voice.map(|voice| {
                        ActiveVoice::from_trigger(
                            trigger.track_id,
                            voice,
                            self.sample_rate,
                            self.frames_per_cycle,
                            &trigger.trigger,
                            trigger.duration_frames,
                        )
                    })
                });
        }
    }

    fn play_transport(&mut self) {
        if self.is_playing {
            return;
        }

        if let Some(routing) = self.pending_routing.take() {
            self.adopt_routing_snapshot(routing)
                .unwrap_or_else(|error| {
                    panic!("pending routing should adopt while playing: {error}")
                });
            self.active_pattern_name = self.pending_pattern_name.take();
        }
        self.next_cycle_boundary_frame = self.frames_per_cycle;
        self.prime_initial_routing = routing_snapshot_has_audio(&self.active_routing);
        self.is_playing = true;
    }

    fn stop_transport(&mut self) {
        if let Some(routing) = self.pending_routing.take() {
            self.adopt_routing_snapshot(routing)
                .unwrap_or_else(|error| {
                    panic!("pending routing should adopt while stopping: {error}")
                });
            self.active_pattern_name = self.pending_pattern_name.take();
        }

        self.scheduler.clear();
        for slot in &mut self.active_voices {
            *slot = None;
        }
        self.current_frame = 0;
        self.current_cycle_start_frame = 0;
        self.next_cycle_boundary_frame = self.frames_per_cycle;
        self.prime_initial_routing = routing_snapshot_has_audio(&self.active_routing);
        self.last_swap_frame = None;
        self.reset_bus_effect_states();
        self.is_playing = false;
    }

    fn resize_mix_buffers(&mut self) {
        self.track_mix_buffer
            .resize(self.active_routing.tracks().len(), (0.0, 0.0));
        self.bus_mix_buffer
            .resize(self.active_routing.buses().len(), (0.0, 0.0));
    }

    fn adopt_routing_snapshot(&mut self, routing: RoutingSnapshot) -> Result<(), EngineError> {
        let old_routing = std::mem::replace(&mut self.active_routing, routing);
        let mut old_bus_effect_states = std::mem::take(&mut self.bus_effect_states);
        let mut bus_effect_states = Vec::with_capacity(self.active_routing.buses().len());

        for bus in self.active_routing.buses() {
            let state = match bus.effect() {
                Some(spec) => {
                    let preserved = old_routing
                        .buses()
                        .iter()
                        .position(|old_bus| {
                            old_bus.name() == bus.name()
                                && bus_effect_specs_match(old_bus.effect(), Some(spec))
                        })
                        .and_then(|index| old_bus_effect_states[index].take());
                    match preserved {
                        Some(mut state) => {
                            state.sync_timing(spec, self.frames_per_cycle)?;
                            Some(state)
                        }
                        None => Some(BusEffectState::from_spec(spec, self.frames_per_cycle)?),
                    }
                }
                None => None,
            };
            bus_effect_states.push(state);
        }

        self.bus_effect_states = bus_effect_states;
        self.resize_mix_buffers();
        Ok(())
    }

    fn sync_bus_effect_timing(&mut self) -> Result<(), EngineError> {
        for (bus, state) in self
            .active_routing
            .buses()
            .iter()
            .zip(self.bus_effect_states.iter_mut())
        {
            if let (Some(spec), Some(state)) = (bus.effect(), state.as_mut()) {
                state.sync_timing(spec, self.frames_per_cycle)?;
            }
        }
        Ok(())
    }

    fn reset_bus_effect_states(&mut self) {
        for state in self.bus_effect_states.iter_mut().flatten() {
            state.reset();
        }
    }

    fn mix_routed_voices(&mut self) -> (f32, f32) {
        self.track_mix_buffer.fill((0.0, 0.0));
        self.bus_mix_buffer.fill((0.0, 0.0));

        for slot in &mut self.active_voices {
            if let Some(voice) = slot.as_mut() {
                if let Some((voice_left, voice_right)) = voice.next_stereo_frame() {
                    let track_index = usize::try_from(voice.track_id().get())
                        .unwrap_or_else(|_| panic!("track id did not fit in usize"));
                    let (left, right) = &mut self.track_mix_buffer[track_index];
                    *left += voice_left;
                    *right += voice_right;
                } else {
                    *slot = None;
                }
            }
        }

        let mut master_left = 0.0_f32;
        let mut master_right = 0.0_f32;

        for track in self.active_routing.tracks() {
            let track_index = usize::try_from(track.id().get())
                .unwrap_or_else(|_| panic!("track id did not fit in usize"));
            let (track_left, track_right) = self.track_mix_buffer[track_index];
            let track_left = track_left * track.level();
            let track_right = track_right * track.level();

            if track.muted() {
                continue;
            }

            if track.routes_to_master() {
                master_left += track_left;
                master_right += track_right;
            }

            for send in track.sends() {
                let bus_index = usize::try_from(send.bus_id().get())
                    .unwrap_or_else(|_| panic!("bus id did not fit in usize"));
                let (bus_left, bus_right) = &mut self.bus_mix_buffer[bus_index];
                *bus_left += track_left * send.level();
                *bus_right += track_right * send.level();
            }
        }

        for bus in self.active_routing.buses() {
            if !bus.routes_to_master() {
                continue;
            }

            let bus_index = usize::try_from(bus.id().get())
                .unwrap_or_else(|_| panic!("bus id did not fit in usize"));
            let (bus_left, bus_right) = self.bus_mix_buffer[bus_index];
            if let Some(effect) = self.bus_effect_states[bus_index].as_mut() {
                let (wet_left, wet_right) = effect.process_frame(bus_left, bus_right);
                master_left += wet_left;
                master_right += wet_right;
            } else {
                master_left += bus_left;
                master_right += bus_right;
            }
        }

        (master_left.clamp(-1.0, 1.0), master_right.clamp(-1.0, 1.0))
    }
}

/// Render-side engine state intended to live on the audio thread.
#[derive(Debug)]
pub struct RenderEngine {
    command_rx: Consumer<EngineCommand>,
    core: EngineCore,
    transport: Arc<SharedTransport>,
}

impl RenderEngine {
    fn new(
        command_rx: Consumer<EngineCommand>,
        config: &StreamConfig,
        transport: Arc<SharedTransport>,
    ) -> Result<Self, EngineError> {
        let engine = Self {
            command_rx,
            core: EngineCore::new(config)?,
            transport,
        };
        engine.publish_transport();
        Ok(engine)
    }

    /// Applies any queued commands and renders into an interleaved output buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if command application fails or if `output` does not
    /// contain a whole number of frames for this renderer's channel layout.
    pub fn render_into_interleaved(&mut self, output: &mut [f32]) -> Result<(), EngineError> {
        self.drain_commands()?;
        let result = self.core.render_into_interleaved(output);
        self.publish_transport();
        result
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
        self.core.active_pattern_name.as_deref()
    }

    /// Returns the active routed track names in snapshot order.
    #[must_use]
    pub fn active_track_names_for_test(&self) -> Vec<&str> {
        self.core
            .active_routing
            .tracks()
            .iter()
            .map(crate::routing::TrackState::name)
            .collect()
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

    fn publish_transport(&self) {
        self.transport.publish(&self.core);
    }
}

/// UI-side command producer for the current minimal engine slice.
#[derive(Debug)]
pub struct EngineHandle {
    command_tx: Producer<EngineCommand>,
    test_renderer: Option<RenderEngine>,
    transport: Arc<SharedTransport>,
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
                    && left.core.active_routing == right.core.active_routing
                    && left.core.pending_routing == right.core.pending_routing
                    && left.core.active_pattern_name == right.core.active_pattern_name
                    && left.core.pending_pattern_name == right.core.pending_pattern_name
                    && left.core.prime_initial_routing == right.core.prime_initial_routing
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
        let transport = Arc::new(SharedTransport::default());
        let renderer = RenderEngine::new(command_rx, config, Arc::clone(&transport))?;
        Ok((
            Self {
                command_tx,
                test_renderer: None,
                transport,
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

    /// Returns the active routed track names for the embedded test renderer.
    ///
    /// # Panics
    ///
    /// Panics if the handle does not own an embedded test renderer.
    #[must_use]
    pub fn active_track_names_for_test(&self) -> Vec<&str> {
        self.test_renderer_ref().active_track_names_for_test()
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

    /// Returns a UI-readable transport snapshot for the current engine state.
    #[must_use]
    pub fn transport_snapshot(&self) -> TransportSnapshot {
        self.transport.snapshot()
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

/// Calculates the number of audio frames required to render exactly one pattern cycle.
///
/// This is used by both the real-time audio thread and the offline renderer to map
/// musical time (cycles) to discrete DSP time (frames).
///
/// # Parameters
/// - `sample_rate`: The number of frames per second (e.g., 44100).
/// - `tempo_bpm`: The current tempo in Beats Per Minute.
///
/// # Errors
///
/// Returns [`EngineError::InvalidTempo`] if `tempo_bpm` is zero, negative, or not finite.
/// Returns [`EngineError::FrameOverflow`] if the calculated frames exceed `u64::MAX`.
///
/// # Examples
///
/// ```
/// use orpheus_dsp::{EngineError, frames_per_cycle};
///
/// let frames = frames_per_cycle(44100, 120.0).unwrap();
/// assert_eq!(frames, 88200); // 120 BPM = 2 beats/sec = 4 beats/cycle = 2 seconds/cycle
/// ```
pub fn frames_per_cycle(sample_rate: u32, tempo_bpm: f32) -> Result<u64, EngineError> {
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

fn default_main_routing_snapshot() -> RoutingSnapshot {
    RoutingSnapshot::builder()
        .main_track()
        .build()
        .unwrap_or_else(|error| panic!("default routing snapshot must be valid: {error}"))
}

fn compatibility_routing_snapshot(pattern: &PatternUpdate) -> RoutingSnapshot {
    RoutingSnapshot::builder()
        .track_with_source(
            "main",
            TrackSource::SamplePattern(pattern.events().to_vec().into_boxed_slice()),
        )
        .route("main", "master")
        .build()
        .unwrap_or_else(|error| panic!("compatibility routing snapshot must be valid: {error}"))
}

fn routing_snapshot_has_audio(snapshot: &RoutingSnapshot) -> bool {
    snapshot
        .tracks()
        .iter()
        .any(|track| matches!(track.source(), TrackSource::SamplePattern(_)))
}

fn bus_effect_specs_match(left: Option<&BusEffectSpec>, right: Option<&BusEffectSpec>) -> bool {
    left == right
}
