//! The `engine` module encapsulates the real-time audio rendering system.
//!
//! This module coordinates the interaction between system audio APIs (like ALSA or `CoreAudio`),
//! lock-free command queues, and the internal voice scheduler. It provides a thread-safe
//! `EngineHandle` for the front-end to control the `RenderEngine` running on the audio thread.

use cpal::{BufferSize, SampleRate, StreamConfig};
use rtrb::{Consumer, Producer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use thiserror::Error;

use orpheus_pattern::Event;

use crate::command::{EngineCommand, PatternUpdate, SampleTrigger, new_command_queue};
use crate::effects::BusEffectState;
use crate::graph_voice::{
    GraphVoiceBank, graph_note_params, graph_note_voice_params, graph_note_voice_ramps,
};
use crate::plugin_host::PluginProcessor;
use crate::routing::{BusEffectSpec, RoutingSnapshot, TrackSource};
use crate::sample_bank::SampleBank;
use crate::scheduler::Scheduler;
use crate::voice::{ActiveVoice, DEFAULT_ANALOG_BASE_FREQUENCY_HZ};

pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;
const DEFAULT_CHANNELS: u16 = 2;
pub const DEFAULT_TEMPO_BPM: f32 = 120.0;
const BEATS_PER_CYCLE: f64 = 4.0;
const MAX_ACTIVE_VOICES: usize = 32;
/// Number of engine-side generator slots addressable by
/// [`crate::routing::GeneratorId`] (ADR 0009). Slots are preallocated at
/// engine construction so the audio thread never grows the table.
pub const MAX_GENERATORS: usize = 8;

/// Number of per-track level-meter slots (ADR 0013).
///
/// The meter storage is a fixed-size atomic array indexed by
/// [`crate::routing::TrackId`], preallocated so the audio thread only ever
/// stores into it — never grows it. Track ids at or above this ceiling are
/// simply not metered.
pub const MAX_METERED_TRACKS: usize = 64;

/// Half-life, in seconds, of the peak-hold meter decay (ADR 0013). The held
/// peak falls to half its value over this span of silence, giving a natural
/// VU/peak-hold feel independent of tempo or render-block size.
const METER_DECAY_HALF_LIFE_SECS: f32 = 0.2;

/// Folds a live sample `magnitude` into a running peak-hold meter value.
///
/// The previous peak decays by `decay` (a per-frame factor in `(0, 1)`) and is
/// then pinned back up by the current sample via `max`, so a fresh louder
/// sample wins instantly while silence lets the peak fall smoothly toward zero.
/// This is the pure kernel of the lock-free metering path (ADR 0013).
///
/// # Examples
///
/// ```
/// use orpheus_dsp::decayed_peak;
///
/// // A louder sample wins immediately.
/// assert!((decayed_peak(0.1, 0.8, 0.99) - 0.8).abs() < f32::EPSILON);
/// // Silence decays the held peak by exactly the factor.
/// assert!((decayed_peak(0.8, 0.0, 0.5) - 0.4).abs() < f32::EPSILON);
/// ```
#[must_use]
pub fn decayed_peak(previous: f32, magnitude: f32, decay: f32) -> f32 {
    (previous * decay).max(magnitude)
}

/// Computes the per-frame peak-meter decay factor for a given `sample_rate`.
///
/// Derived once at engine construction so the meter ballistics stay constant
/// regardless of tempo or render-block size (ADR 0013): the returned factor
/// halves a held peak over `METER_DECAY_HALF_LIFE_SECS` seconds of silence.
///
/// # Examples
///
/// ```
/// use orpheus_dsp::meter_decay_per_frame;
///
/// let decay = meter_decay_per_frame(48_000);
/// assert!(decay > 0.99 && decay < 1.0);
/// ```
#[must_use]
// The sample rate is a small integer far within f32's exact-integer range, and
// the decay factor is a display-oriented ballistics constant.
#[allow(clippy::cast_precision_loss)]
pub fn meter_decay_per_frame(sample_rate: u32) -> f32 {
    if sample_rate == 0 {
        return 0.0;
    }
    let frames = METER_DECAY_HALF_LIFE_SECS * sample_rate as f32;
    0.5_f32.powf(1.0 / frames)
}

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
    /// A monotonically increasing counter representing the number of successful routing snapshot replacements.
    #[must_use]
    pub const fn publish_epoch(&self) -> u64 {
        self.publish_epoch
    }

    /// The absolute count of frames processed since the audio engine started.
    #[must_use]
    pub const fn current_frame(&self) -> u64 {
        self.current_frame
    }

    /// The frame index representing the boundary where the current active cycle began.
    #[must_use]
    pub const fn current_cycle_start_frame(&self) -> u64 {
        self.current_cycle_start_frame
    }

    /// The length of a single musical cycle expressed in audio frames, derived from the active tempo.
    #[must_use]
    pub const fn frames_per_cycle(&self) -> u64 {
        self.frames_per_cycle
    }

    /// The transport speed expressed in Beats Per Minute (BPM).
    #[must_use]
    pub const fn tempo_bpm(&self) -> f32 {
        f32::from_bits(self.tempo_bpm_bits)
    }

    /// Indicates whether the transport is advancing time and triggering events (`true`) or halted (`false`).
    #[must_use]
    pub const fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Indicates that a `LoadPattern` command was received but the engine is waiting for the next cycle boundary to apply it.
    #[must_use]
    pub const fn has_pending_pattern(&self) -> bool {
        self.has_pending_pattern
    }

    /// Indicates that a `SwapRoutingSnapshot` command was received but the engine is waiting for the next cycle boundary to apply it.
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
    is_poisoned: AtomicBool,
}

struct PublishGuard<'a> {
    transport: &'a SharedTransport,
    completed: bool,
}

impl Drop for PublishGuard<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.transport.is_poisoned.store(true, Ordering::Release);
        }
    }
}

impl SharedTransport {
    fn publish(&self, core: &EngineCore) {
        // Start the write transaction. Relaxed is sufficient because the
        // atomic fence handles the required release semantics.
        self.publish_epoch.fetch_add(1, Ordering::Relaxed);
        std::sync::atomic::fence(Ordering::Release);

        let mut guard = PublishGuard {
            transport: self,
            completed: false,
        };

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

        // Disarm the guard as we have completed successfully.
        guard.completed = true;

        // Commit the write transaction.
        std::sync::atomic::fence(Ordering::Release);
        self.publish_epoch.fetch_add(1, Ordering::Relaxed);
    }

    fn snapshot(&self) -> TransportSnapshot {
        loop {
            if self.is_poisoned.load(Ordering::Acquire) {
                return TransportSnapshot {
                    publish_epoch: 0,
                    current_frame: 0,
                    current_cycle_start_frame: 0,
                    frames_per_cycle: 0,
                    tempo_bpm_bits: 0,
                    is_playing: false,
                    has_pending_pattern: false,
                    has_pending_routing: false,
                };
            }

            let start_epoch = self.publish_epoch.load(Ordering::Relaxed);
            std::sync::atomic::fence(Ordering::Acquire);

            // If odd, a write is in progress. Wait for it to finish.
            if !start_epoch.is_multiple_of(2) {
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

            let end_epoch = self.publish_epoch.load(Ordering::Acquire);
            // If the epoch is unchanged, we observed a consistent state.
            if start_epoch == end_epoch {
                return snap;
            }
        }
    }
}

/// A UI-readable snapshot of the per-track signal level meters (ADR 0013).
///
/// Each entry is the current peak-hold amplitude (linear, typically `0.0..=1.0`)
/// for the track with the matching [`crate::routing::TrackId`]. The snapshot is
/// read lock-free from the audio thread via [`EngineHandle::meter_snapshot`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LevelSnapshot {
    peaks: [f32; MAX_METERED_TRACKS],
}

impl LevelSnapshot {
    /// Returns the peak amplitude for the track at `index`, or `0.0` when the
    /// index is outside the metered range.
    #[must_use]
    pub fn track_peak(&self, index: usize) -> f32 {
        self.peaks.get(index).copied().unwrap_or(0.0)
    }

    /// Returns the full fixed-size slice of per-track peak amplitudes.
    #[must_use]
    pub const fn peaks(&self) -> &[f32] {
        &self.peaks
    }
}

/// Lock-free, fixed-capacity per-track meter storage shared between the audio
/// thread (which stores) and the UI (which loads). See ADR 0013.
#[derive(Debug)]
struct SharedMeters {
    peaks: [AtomicU32; MAX_METERED_TRACKS],
}

impl Default for SharedMeters {
    fn default() -> Self {
        Self {
            peaks: std::array::from_fn(|_| AtomicU32::new(0)),
        }
    }
}

impl SharedMeters {
    /// Publishes the audio thread's running peak accumulator into the shared
    /// atomics. Mirrors [`SharedTransport::publish`]: `f32` bits stored with
    /// `Relaxed` ordering, no locks and no allocation.
    fn publish(&self, meter_peaks: &[f32; MAX_METERED_TRACKS]) {
        for (slot, peak) in self.peaks.iter().zip(meter_peaks.iter()) {
            slot.store(peak.to_bits(), Ordering::Relaxed);
        }
    }

    /// Reads a consistent-enough snapshot of every track's peak. Each slot is a
    /// single `u32`, so an individual `f32` never tears; cross-track skew is
    /// imperceptible for a moving meter (ADR 0013).
    fn snapshot(&self) -> LevelSnapshot {
        LevelSnapshot {
            peaks: std::array::from_fn(|index| {
                f32::from_bits(self.peaks[index].load(Ordering::Relaxed))
            }),
        }
    }
}

/// Errors raised by the minimal Orpheus audio engine.
///
/// **Recovery:** Catch the error and display it to the user. Typical failures relate to system audio backend misconfiguration or extreme temporal values causing frame overflows. Resetting the audio backend or restarting the application might be necessary for serious host faults.
///
/// # Examples
///
/// ```
/// use orpheus_dsp::EngineError;
///
/// let err = EngineError::InvalidChannelCount;
/// assert_eq!(err.to_string(), "audio output must have at least one channel");
/// ```
#[derive(Debug, Error)]
pub enum EngineError {
    /// The audio backend provided an invalid number of channels.
    #[error("audio output must have at least one channel")]
    InvalidChannelCount,
    /// The lock-free queue sending commands to the audio thread is full and cannot accept more messages.
    #[error("engine command queue is full")]
    CommandQueueFull,
    /// The requested tempo is invalid (e.g., zero, negative, or NaN).
    #[error("tempo must be a finite positive value")]
    InvalidTempo,
    /// The requested analog-voice reference frequency is invalid.
    #[error("reference frequency must be a finite positive value")]
    InvalidReferenceFrequency,
    /// A timing calculation resulted in a schedule time earlier than the beginning of the cycle.
    #[error("pattern time produced a negative cycle offset")]
    NegativeCycleOffset,
    /// A sample clock calculation overflowed a 64-bit integer, usually indicating unreachable execution time.
    #[error("sample-clock conversion overflowed the supported range")]
    FrameOverflow,
    /// The engine was asked to play a sample or voice token that does not exist.
    #[error("unknown built-in voice token `{0}`")]
    UnknownVoice(String),
    /// A generator cycle addressed a slot outside the engine's fixed table.
    #[error("generator id must be below {MAX_GENERATORS}")]
    InvalidGeneratorId,
    /// The interleaved output slice length cannot be evenly divided into frames.
    #[error("output buffer length must be a whole number of frames")]
    MisalignedOutputBuffer,
}

#[derive(Debug)]
struct EngineCore {
    scheduler: Scheduler,
    active_voices: Vec<Option<ActiveVoice>>,
    graph_voices: GraphVoiceBank,
    sample_bank: SampleBank,
    active_routing: RoutingSnapshot,
    pending_routing: Option<RoutingSnapshot>,
    bus_effect_states: Vec<Option<BusEffectState>>,
    plugin_processors: Vec<Option<PluginProcessor>>,
    sample_rate: u32,
    channels: usize,
    current_frame: u64,
    tempo_bpm: f32,
    base_hz: f32,
    is_playing: bool,
    frames_per_cycle: u64,
    current_cycle_start_frame: u64,
    next_cycle_boundary_frame: u64,
    active_pattern_name: Option<Box<str>>,
    pending_pattern_name: Option<Box<str>>,
    pending_sample_bank: Option<SampleBank>,
    /// Cycle buffers delivered over the ring but not yet adopted; one fixed
    /// slot per generator id, filled by `PushGeneratorCycle` (latest wins).
    generator_pending: Vec<Option<Box<[Event<SampleTrigger>]>>>,
    /// The buffer each generator slot schedules at cycle boundaries. When no
    /// fresh pending buffer arrives the previous one is re-scheduled, so a
    /// starved generator loops its last cycle instead of falling silent.
    generator_active: Vec<Box<[Event<SampleTrigger>]>>,
    pending_graph_voices: Option<GraphVoiceBank>,
    prime_initial_routing: bool,
    last_swap_frame: Option<u64>,
    track_mix_buffer: Vec<(f32, f32)>,
    bus_mix_buffer: Vec<(f32, f32)>,
    /// Audio-thread-local peak-hold accumulator, one slot per track id (ADR
    /// 0013). Updated per frame with plain `f32` arithmetic and published to
    /// [`SharedMeters`] at the end of each render call.
    meter_peaks: [f32; MAX_METERED_TRACKS],
    /// Per-frame meter decay factor derived from the sample rate at construction.
    meter_decay_per_frame: f32,
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
        let plugin_processors = vec![None; active_routing.tracks().len()];
        Ok(Self {
            scheduler: Scheduler::default(),
            active_voices: std::iter::repeat_with(|| None)
                .take(MAX_ACTIVE_VOICES)
                .collect(),
            // Graph voices are compiled and warmed here, before the audio
            // thread exists; the render path only reuses the pooled voices.
            #[allow(clippy::cast_precision_loss)]
            graph_voices: GraphVoiceBank::with_builtin_programs(config.sample_rate.0 as f32),
            sample_bank: SampleBank::load_builtin(),
            active_routing,
            pending_routing: None,
            bus_effect_states: Vec::new(),
            plugin_processors,
            sample_rate: config.sample_rate.0,
            channels: usize::from(config.channels),
            current_frame: 0,
            tempo_bpm: DEFAULT_TEMPO_BPM,
            base_hz: DEFAULT_ANALOG_BASE_FREQUENCY_HZ,
            is_playing: true,
            frames_per_cycle,
            current_cycle_start_frame: 0,
            next_cycle_boundary_frame: frames_per_cycle,
            active_pattern_name: None,
            pending_pattern_name: None,
            pending_sample_bank: None,
            generator_pending: std::iter::repeat_with(|| None)
                .take(MAX_GENERATORS)
                .collect(),
            generator_active: std::iter::repeat_with(|| Vec::new().into_boxed_slice())
                .take(MAX_GENERATORS)
                .collect(),
            pending_graph_voices: None,
            prime_initial_routing: false,
            last_swap_frame: None,
            track_mix_buffer,
            bus_mix_buffer,
            meter_peaks: [0.0; MAX_METERED_TRACKS],
            meter_decay_per_frame: meter_decay_per_frame(config.sample_rate.0),
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
                self.prime_initial_routing = self.current_frame == 0
                    && (routing_snapshot_has_main_plugin(&snapshot)
                        || routing_snapshot_has_generator(&snapshot));
                self.pending_routing = Some(snapshot);
                self.pending_pattern_name = None;
                Ok(())
            }
            EngineCommand::PushGeneratorCycle(cycle) => {
                let slot = usize::try_from(cycle.generator_id().get())
                    .ok()
                    .filter(|slot| *slot < MAX_GENERATORS)
                    .ok_or(EngineError::InvalidGeneratorId)?;
                self.generator_pending[slot] = Some(cycle.into_events());
                Ok(())
            }
            EngineCommand::ReplaceSampleBank(sample_bank) => {
                self.pending_sample_bank = Some(sample_bank);
                Ok(())
            }
            EngineCommand::ReplaceGraphVoicePrograms(graph_voices) => {
                // The bank arrives fully built and prepared (constructed off
                // the audio thread); adopting it at the boundary is a move.
                self.pending_graph_voices = Some(graph_voices);
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
            EngineCommand::SetReferenceFrequency(base_hz) => {
                if !base_hz.is_finite() || base_hz <= 0.0 {
                    return Err(EngineError::InvalidReferenceFrequency);
                }
                self.base_hz = base_hz;
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

        if let Some(graph_voices) = self.pending_graph_voices.take() {
            // Notes sounding on the outgoing pool end at the swap boundary,
            // mirroring how a replaced sample bank retires its samples.
            self.graph_voices = graph_voices;
        }

        if let Some(routing) = self.pending_routing.take() {
            self.adopt_routing_snapshot(routing)?;
            self.active_pattern_name = self.pending_pattern_name.take();
            self.last_swap_frame = Some(self.current_frame);
        }

        // Adopt freshly delivered generator cycle buffers: a boxed-slice move
        // plus one boundary-time drop of the retired buffer, the same
        // adoption discipline as the routing snapshot swap above.
        for slot in 0..MAX_GENERATORS {
            if let Some(events) = self.generator_pending[slot].take() {
                self.generator_active[slot] = events;
            }
        }

        self.sync_bus_effect_timing()?;
        for processor in self.plugin_processors.iter_mut().flatten() {
            processor.begin_cycle();
        }

        for track in self.active_routing.tracks() {
            match track.source() {
                TrackSource::SamplePattern(events) => {
                    self.scheduler.schedule_cycle_events(
                        track.id(),
                        self.current_cycle_start_frame,
                        self.frames_per_cycle,
                        events.iter(),
                    )?;
                }
                TrackSource::Generator(generator_id) => {
                    let Some(events) = usize::try_from(generator_id.get())
                        .ok()
                        .and_then(|slot| self.generator_active.get(slot))
                    else {
                        return Err(EngineError::InvalidGeneratorId);
                    };
                    self.scheduler.schedule_cycle_events(
                        track.id(),
                        self.current_cycle_start_frame,
                        self.frames_per_cycle,
                        events.iter(),
                    )?;
                }
                TrackSource::Unbound | TrackSource::Plugin(_) => {}
            }
        }

        Ok(())
    }

    fn render_into_interleaved(&mut self, output: &mut [f32]) -> Result<(), EngineError> {
        if !output.len().is_multiple_of(self.channels) {
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
                            self.base_hz,
                        )
                    })
                });
            if slot.is_some() {
                return;
            }
        }

        // Tokens the sample bank and built-in fallbacks do not own may name a
        // pooled graph voice program. The pool is pre-allocated and prepared
        // at construction time, so this path never allocates.
        let token = trigger.trigger.token();
        if self.graph_voices.has_program(token) {
            let (freq_hz, gain, pan) = graph_note_params(&trigger.trigger, self.base_hz);
            let params = graph_note_voice_params(&trigger.trigger);
            let ramps = graph_note_voice_ramps(&trigger.trigger, trigger.duration_frames, &params);
            let _ = self.graph_voices.trigger_with_automation(
                token,
                trigger.track_id,
                trigger.duration_frames,
                freq_hz,
                gain,
                pan,
                params,
                &ramps,
            );
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
        self.graph_voices.stop_all();
        self.current_frame = 0;
        self.current_cycle_start_frame = 0;
        self.next_cycle_boundary_frame = self.frames_per_cycle;
        self.prime_initial_routing = routing_snapshot_has_audio(&self.active_routing);
        self.last_swap_frame = None;
        self.reset_bus_effect_states();
        // Meters fall to silence when the transport stops (ADR 0013).
        self.meter_peaks = [0.0; MAX_METERED_TRACKS];
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
        self.plugin_processors = self
            .active_routing
            .tracks()
            .iter()
            .map(|track| match track.source() {
                TrackSource::Plugin(source) => Some(PluginProcessor::new(source, self.sample_rate)),
                TrackSource::Unbound
                | TrackSource::SamplePattern(_)
                | TrackSource::Generator(_) => None,
            })
            .collect();
        self.resize_mix_buffers();
        // Retire meter slots for tracks that no longer exist so they do not
        // freeze at a stale peak (ADR 0013).
        for peak in self
            .meter_peaks
            .iter_mut()
            .skip(self.active_routing.tracks().len())
        {
            *peak = 0.0;
        }
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

        self.graph_voices.render_frame(&mut self.track_mix_buffer);

        let local_frame = self
            .current_frame
            .saturating_sub(self.current_cycle_start_frame);
        for track in self.active_routing.tracks() {
            let TrackSource::Plugin(source) = track.source() else {
                continue;
            };
            let track_index = usize::try_from(track.id().get())
                .unwrap_or_else(|_| panic!("track id did not fit in usize"));
            let Some(processor) = self.plugin_processors[track_index].as_mut() else {
                continue;
            };
            let (plugin_left, plugin_right) =
                processor.process_frame(source, local_frame, self.frames_per_cycle);
            let (left, right) = &mut self.track_mix_buffer[track_index];
            *left += plugin_left;
            *right += plugin_right;
        }

        let mut master_left = 0.0_f32;
        let mut master_right = 0.0_f32;

        for track in self.active_routing.tracks() {
            let track_index = usize::try_from(track.id().get())
                .unwrap_or_else(|_| panic!("track id did not fit in usize"));
            let (track_left, track_right) = self.track_mix_buffer[track_index];
            let track_left = track_left * track.level();
            let track_right = track_right * track.level();

            // Meter the post-fader signal (ADR 0013). A muted track feeds a
            // magnitude of 0.0 so its peak decays away rather than lingering.
            if track_index < MAX_METERED_TRACKS {
                let magnitude = if track.muted() {
                    0.0
                } else {
                    track_left.abs().max(track_right.abs())
                };
                self.meter_peaks[track_index] = decayed_peak(
                    self.meter_peaks[track_index],
                    magnitude,
                    self.meter_decay_per_frame,
                );
            }

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
                *bus_left = track_left.mul_add(send.level(), *bus_left);
                *bus_right = track_right.mul_add(send.level(), *bus_right);
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
    meters: Arc<SharedMeters>,
}

impl RenderEngine {
    fn new(
        command_rx: Consumer<EngineCommand>,
        config: &StreamConfig,
        transport: Arc<SharedTransport>,
        meters: Arc<SharedMeters>,
    ) -> Result<Self, EngineError> {
        let engine = Self {
            command_rx,
            core: EngineCore::new(config)?,
            transport,
            meters,
        };
        engine.publish_transport();
        engine.publish_meters();
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
        self.publish_meters();
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

    /// Inspects the lock-free data structures to read the active pattern name.
    ///
    /// This method is designed exclusively for testing the core engine logic to ensure
    /// that pattern swaps occur atomically at exactly the right frame boundaries without
    /// dropping the audio thread's execution cadence.
    #[must_use]
    pub fn active_pattern_name_for_test(&self) -> Option<&str> {
        self.core.active_pattern_name.as_deref()
    }

    /// Inspects the lock-free routing data to read the active track names in snapshot order.
    ///
    /// The mixer topology is swapped atomically at cycle boundaries. This test-only method
    /// validates that the `OfflineRenderer` correctly digested routing commands and applied
    /// them without tearing the dependency graph.
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

    /// Exposes the engine's current analog-voice reference frequency in Hertz.
    ///
    /// Drains pending commands so a recently enqueued
    /// [`EngineCommand::SetReferenceFrequency`] is reflected before the caller reads.
    ///
    /// # Panics
    ///
    /// Panics if draining queued commands fails.
    #[must_use]
    pub fn reference_frequency_hz_for_test(&mut self) -> f32 {
        self.drain_commands()
            .unwrap_or_else(|error| panic!("drain for ref-freq read failed: {error}"));
        self.core.base_hz
    }

    /// Exposes the engine's internal continuous-time synchronization metric.
    ///
    /// `orpheus-dsp` achieves sample-accurate musical timing by determining exactly how many
    /// audio frames comprise a full musical cycle. This getter ensures unit tests can verify
    /// that tempo changes correctly modulate the `frames_per_cycle` property.
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

    fn publish_meters(&self) {
        self.meters.publish(&self.core.meter_peaks);
    }
}

/// UI-side command producer for the current minimal engine slice.
#[derive(Debug)]
pub struct EngineHandle {
    command_tx: Producer<EngineCommand>,
    test_renderer: Option<RenderEngine>,
    transport: Arc<SharedTransport>,
    meters: Arc<SharedMeters>,
    sample_rate: u32,
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
        let meters = Arc::new(SharedMeters::default());
        let renderer = RenderEngine::new(
            command_rx,
            config,
            Arc::clone(&transport),
            Arc::clone(&meters),
        )?;
        Ok((
            Self {
                command_tx,
                test_renderer: None,
                transport,
                meters,
                sample_rate: config.sample_rate.0,
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

    /// The output sample rate this handle's engine renders at, in Hertz.
    ///
    /// Off-thread program builders (sample-accurate graph voice banks, for
    /// example) use this so compiled DSP state matches the render rate.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub const fn sample_rate_hz(&self) -> f32 {
        self.sample_rate as f32
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

    /// Inspects the lock-free data structures of the embedded test renderer to read the active pattern name.
    ///
    /// This method allows `orpheus-lang` tests to verify that `EngineHandle` commands
    /// correctly reach and modify the underlying test renderer's state across cycle boundaries.
    ///
    /// # Panics
    ///
    /// Panics if the handle does not own an embedded test renderer.
    #[must_use]
    pub fn active_pattern_name_for_test(&self) -> Option<&str> {
        self.test_renderer_ref().active_pattern_name_for_test()
    }

    /// Inspects the embedded test renderer's lock-free routing data to read the active track names.
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

    /// Exposes the embedded test renderer's internal continuous-time synchronization metric.
    ///
    /// # Panics
    ///
    /// Panics if the handle does not own an embedded test renderer.
    #[must_use]
    pub fn frames_per_cycle_for_test(&self) -> u64 {
        self.test_renderer_ref().frames_per_cycle_for_test()
    }

    /// Exposes the embedded test renderer's analog-voice reference frequency in Hertz.
    ///
    /// Drains pending commands first so the most recently enqueued
    /// [`EngineCommand::SetReferenceFrequency`] is observable.
    ///
    /// # Panics
    ///
    /// Panics if the handle does not own an embedded test renderer.
    #[must_use]
    pub fn reference_frequency_hz_for_test(&mut self) -> f32 {
        self.test_renderer_mut().reference_frequency_hz_for_test()
    }

    /// Returns a UI-readable transport snapshot for the current engine state.
    #[must_use]
    pub fn transport_snapshot(&self) -> TransportSnapshot {
        self.transport.snapshot()
    }

    /// Returns a UI-readable per-track level-meter snapshot (ADR 0013).
    ///
    /// The meters are updated on the audio thread and read here lock-free, the
    /// same discipline as [`EngineHandle::transport_snapshot`].
    #[must_use]
    pub fn meter_snapshot(&self) -> LevelSnapshot {
        self.meters.snapshot()
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
/// ```ignore
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

    let cycle_duration = Duration::try_from_secs_f64(cycle_seconds).unwrap_or(Duration::MAX);
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
    snapshot.tracks().iter().any(|track| {
        matches!(
            track.source(),
            TrackSource::SamplePattern(_) | TrackSource::Plugin(_) | TrackSource::Generator(_)
        )
    })
}

fn routing_snapshot_has_generator(snapshot: &RoutingSnapshot) -> bool {
    snapshot
        .tracks()
        .iter()
        .any(|track| matches!(track.source(), TrackSource::Generator(_)))
}

fn routing_snapshot_has_main_plugin(snapshot: &RoutingSnapshot) -> bool {
    snapshot
        .tracks()
        .iter()
        .any(|track| track.name() == "main" && matches!(track.source(), TrackSource::Plugin(_)))
}

fn bus_effect_specs_match(left: Option<&BusEffectSpec>, right: Option<&BusEffectSpec>) -> bool {
    left == right
}
