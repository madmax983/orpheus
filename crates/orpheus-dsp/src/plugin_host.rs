//! Headless plugin-hosting primitives for VST3/AU style instrument tracks.
//!
//! Phase 1 keeps the binary plugin backend behind immutable descriptors and a
//! real-time-safe render facade. The built-in processor is deterministic and
//! headless, which lets the mixer, scheduler, and language integration prove the
//! contract before a vendor SDK backend is attached.

use std::path::PathBuf;

use orpheus_pattern::{Event, Rational};
use thiserror::Error;

const MAX_PLUGIN_VOICES: usize = 32;
const DEFAULT_PLUGIN_GAIN: f32 = 1.0;
const PLUGIN_OUTPUT_TRIM: f32 = 0.18;

/// Supported plugin binary families.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PluginFormat {
    /// Steinberg VST3 bundle or component.
    Vst3,
    /// Apple `AudioUnit` component.
    AudioUnit,
}

/// Errors raised while constructing immutable plugin-host data.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PluginHostError {
    /// A plugin identifier was empty or whitespace-only.
    #[error("plugin identifier must not be empty")]
    EmptyIdentifier,
    /// A MIDI note was outside the valid seven-bit MIDI range.
    #[error("plugin note number must be within [0, 127]")]
    InvalidNoteNumber,
    /// A note velocity was outside the normalized MIDI velocity range.
    #[error("plugin note velocity must be finite and within [0, 1]")]
    InvalidVelocity,
    /// A plugin parameter name was empty or whitespace-only.
    #[error("plugin parameter name must not be empty")]
    EmptyParameterName,
    /// A plugin automation value was outside the normalized host parameter range.
    #[error("plugin parameter values must be finite and within [0, 1]")]
    InvalidParameterValue,
}

/// Immutable description of a plugin instance requested by the language layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PluginDescriptor {
    format: PluginFormat,
    identifier: Box<str>,
    search_paths: Box<[PathBuf]>,
}

impl PluginDescriptor {
    /// Creates a VST3 descriptor resolved against the platform's standard paths.
    ///
    /// # Panics
    ///
    /// Panics if `identifier` is empty. Use [`Self::try_new`] when accepting
    /// untrusted user input.
    #[must_use]
    pub fn vst3(identifier: impl Into<Box<str>>) -> Self {
        Self::try_new(PluginFormat::Vst3, identifier)
            .unwrap_or_else(|error| panic!("invalid VST3 plugin descriptor: {error}"))
    }

    /// Creates an `AudioUnit` descriptor resolved against the platform's standard paths.
    ///
    /// # Panics
    ///
    /// Panics if `identifier` is empty. Use [`Self::try_new`] when accepting
    /// untrusted user input.
    #[must_use]
    pub fn audio_unit(identifier: impl Into<Box<str>>) -> Self {
        Self::try_new(PluginFormat::AudioUnit, identifier)
            .unwrap_or_else(|error| panic!("invalid AU plugin descriptor: {error}"))
    }

    /// Creates a descriptor for the selected plugin format.
    ///
    /// # Errors
    ///
    /// Returns [`PluginHostError::EmptyIdentifier`] if `identifier` is blank.
    pub fn try_new(
        format: PluginFormat,
        identifier: impl Into<Box<str>>,
    ) -> Result<Self, PluginHostError> {
        let identifier = identifier.into();
        if identifier.trim().is_empty() {
            return Err(PluginHostError::EmptyIdentifier);
        }
        Ok(Self {
            format,
            identifier,
            search_paths: default_search_paths(format),
        })
    }

    /// The binary plugin family this descriptor targets.
    #[must_use]
    pub const fn format(&self) -> PluginFormat {
        self.format
    }

    /// User-facing plugin identifier or explicit bundle path.
    #[must_use]
    pub fn identifier(&self) -> &str {
        &self.identifier
    }

    /// Standard host search paths captured off the audio thread.
    #[must_use]
    pub fn search_paths(&self) -> &[PathBuf] {
        &self.search_paths
    }
}

/// A single MIDI note event delivered to a plugin instrument.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PluginNote {
    note_number: u8,
    velocity: f32,
    channel: u8,
}

impl PluginNote {
    /// Creates a normalized note event on MIDI channel 1.
    ///
    /// # Errors
    ///
    /// Returns [`PluginHostError::InvalidVelocity`] if `velocity` is not finite
    /// or outside `[0, 1]`.
    pub fn new(note_number: u8, velocity: f32) -> Result<Self, PluginHostError> {
        if !velocity.is_finite() || !(0.0..=1.0).contains(&velocity) {
            return Err(PluginHostError::InvalidVelocity);
        }
        Ok(Self {
            note_number,
            velocity,
            channel: 0,
        })
    }

    /// The MIDI note number.
    #[must_use]
    pub const fn note_number(self) -> u8 {
        self.note_number
    }

    /// The normalized MIDI velocity.
    #[must_use]
    pub const fn velocity(self) -> f32 {
        self.velocity
    }

    /// Zero-based MIDI channel.
    #[must_use]
    pub const fn channel(self) -> u8 {
        self.channel
    }
}

/// A named host-automation lane for a plugin parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginParameterLane {
    name: Box<str>,
    events: Box<[Event<f32>]>,
}

impl PluginParameterLane {
    /// Creates a parameter automation lane using normalized values.
    ///
    /// # Errors
    ///
    /// Returns [`PluginHostError::EmptyParameterName`] for blank names and
    /// [`PluginHostError::InvalidParameterValue`] for non-finite or out-of-range
    /// event values.
    pub fn new(
        name: impl Into<Box<str>>,
        events: Box<[Event<f32>]>,
    ) -> Result<Self, PluginHostError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(PluginHostError::EmptyParameterName);
        }
        if events
            .iter()
            .any(|event| !event.value.is_finite() || !(0.0..=1.0).contains(&event.value))
        {
            return Err(PluginHostError::InvalidParameterValue);
        }
        Ok(Self { name, events })
    }

    /// The automatable parameter name as presented by the plugin.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Normalized automation events for this parameter.
    #[must_use]
    pub fn events(&self) -> &[Event<f32>] {
        &self.events
    }
}

/// Fully materialized plugin track input consumed by the render thread.
#[derive(Clone, Debug, PartialEq)]
pub struct PluginTrackSource {
    descriptor: PluginDescriptor,
    notes: Box<[Event<PluginNote>]>,
    parameter_lanes: Box<[PluginParameterLane]>,
}

impl PluginTrackSource {
    /// Creates a plugin track source with no note or automation events.
    #[must_use]
    pub fn new(descriptor: PluginDescriptor) -> Self {
        Self {
            descriptor,
            notes: Box::default(),
            parameter_lanes: Box::default(),
        }
    }

    /// Replaces the track's MIDI note schedule.
    #[must_use]
    pub fn with_notes(mut self, notes: Box<[Event<PluginNote>]>) -> Self {
        self.notes = notes;
        self
    }

    /// Replaces the track's host-automation lanes.
    #[must_use]
    pub fn with_parameter_lanes(mut self, parameter_lanes: Box<[PluginParameterLane]>) -> Self {
        self.parameter_lanes = parameter_lanes;
        self
    }

    /// Appends a host-automation lane.
    #[must_use]
    pub fn with_parameter_lane(mut self, lane: PluginParameterLane) -> Self {
        let mut lanes = Vec::with_capacity(self.parameter_lanes.len() + 1);
        lanes.extend(self.parameter_lanes.iter().cloned());
        lanes.push(lane);
        self.parameter_lanes = lanes.into_boxed_slice();
        self
    }

    /// The immutable plugin descriptor.
    #[must_use]
    pub const fn descriptor(&self) -> &PluginDescriptor {
        &self.descriptor
    }

    /// Scheduled MIDI notes for one Orpheus cycle.
    #[must_use]
    pub fn notes(&self) -> &[Event<PluginNote>] {
        &self.notes
    }

    /// Scheduled parameter automation lanes for one Orpheus cycle.
    #[must_use]
    pub fn parameter_lanes(&self) -> &[PluginParameterLane] {
        &self.parameter_lanes
    }
}

/// Test-visible capacities for render-path allocation checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PluginBufferCapacities {
    /// Backing storage capacity for plugin voices.
    pub voice_capacity: usize,
    /// Backing storage capacity for per-parameter event cursors.
    pub parameter_cursor_capacity: usize,
    /// Backing storage capacity for current normalized parameter values.
    pub parameter_value_capacity: usize,
}

/// Real-time render state for one headless plugin track.
#[derive(Clone, Debug)]
pub struct PluginProcessor {
    sample_rate_hz: f32,
    next_note_index: usize,
    parameter_cursors: Vec<usize>,
    parameter_values: Vec<f32>,
    gain_lane_index: Option<usize>,
    voices: Vec<Option<PluginVoice>>,
}

#[derive(Clone, Copy, Debug)]
struct PluginVoice {
    phase: f32,
    phase_step: f32,
    velocity: f32,
    remaining_frames: u32,
}

impl PluginProcessor {
    /// Creates preallocated render state for a plugin track.
    #[must_use]
    #[expect(
        clippy::cast_precision_loss,
        reason = "supported audio sample rates are exactly representable as f32"
    )]
    pub fn new(source: &PluginTrackSource, sample_rate_hz: u32) -> Self {
        let parameter_count = source.parameter_lanes().len();
        let gain_lane_index = source
            .parameter_lanes()
            .iter()
            .position(|lane| lane.name().eq_ignore_ascii_case("gain"));
        Self {
            sample_rate_hz: sample_rate_hz as f32,
            next_note_index: 0,
            parameter_cursors: vec![0; parameter_count],
            parameter_values: vec![DEFAULT_PLUGIN_GAIN; parameter_count],
            gain_lane_index,
            voices: vec![None; MAX_PLUGIN_VOICES],
        }
    }

    /// Resets cycle-local event cursors without reallocating.
    pub fn begin_cycle(&mut self) {
        self.next_note_index = 0;
        self.parameter_cursors.fill(0);
        self.parameter_values.fill(DEFAULT_PLUGIN_GAIN);
    }

    /// Processes one stereo frame for a plugin track.
    #[must_use]
    pub fn process_frame(
        &mut self,
        source: &PluginTrackSource,
        local_frame: u64,
        frames_per_cycle: u64,
    ) -> (f32, f32) {
        self.apply_due_parameter_events(source, local_frame, frames_per_cycle);
        self.activate_due_notes(source, local_frame, frames_per_cycle);

        let gain = self
            .gain_lane_index
            .and_then(|index| self.parameter_values.get(index))
            .copied()
            .unwrap_or(DEFAULT_PLUGIN_GAIN);
        let mut mono = 0.0_f32;

        for slot in &mut self.voices {
            let Some(voice) = slot.as_mut() else {
                continue;
            };
            if voice.remaining_frames == 0 {
                *slot = None;
                continue;
            }

            let sample = voice.phase.sin() * voice.velocity * gain * PLUGIN_OUTPUT_TRIM;
            mono += sample;
            voice.phase += voice.phase_step;
            voice.remaining_frames = voice.remaining_frames.saturating_sub(1);
        }

        (mono.clamp(-1.0, 1.0), (mono * 0.98).clamp(-1.0, 1.0))
    }

    /// Returns render-state capacities for tests that guard against process-path growth.
    #[doc(hidden)]
    #[must_use]
    pub const fn buffer_capacities_for_test(&self) -> PluginBufferCapacities {
        PluginBufferCapacities {
            voice_capacity: self.voices.capacity(),
            parameter_cursor_capacity: self.parameter_cursors.capacity(),
            parameter_value_capacity: self.parameter_values.capacity(),
        }
    }

    fn apply_due_parameter_events(
        &mut self,
        source: &PluginTrackSource,
        local_frame: u64,
        frames_per_cycle: u64,
    ) {
        for (lane_index, lane) in source.parameter_lanes().iter().enumerate() {
            if lane_index >= self.parameter_cursors.len()
                || lane_index >= self.parameter_values.len()
            {
                continue;
            }
            let cursor = &mut self.parameter_cursors[lane_index];
            while let Some(event) = lane.events().get(*cursor) {
                let Some(event_frame) =
                    rational_to_frame_offset(event.part.start(), frames_per_cycle)
                else {
                    *cursor = cursor.saturating_add(1);
                    continue;
                };
                if event_frame > local_frame {
                    break;
                }
                self.parameter_values[lane_index] = event.value;
                *cursor = cursor.saturating_add(1);
            }
        }
    }

    fn activate_due_notes(
        &mut self,
        source: &PluginTrackSource,
        local_frame: u64,
        frames_per_cycle: u64,
    ) {
        while let Some(event) = source.notes().get(self.next_note_index) {
            let Some(event_frame) = rational_to_frame_offset(event.part.start(), frames_per_cycle)
            else {
                self.next_note_index = self.next_note_index.saturating_add(1);
                continue;
            };
            if event_frame > local_frame {
                break;
            }
            let duration = note_duration_frames(event, frames_per_cycle);
            self.activate_voice(event.value, duration);
            self.next_note_index = self.next_note_index.saturating_add(1);
        }
    }

    fn activate_voice(&mut self, note: PluginNote, duration_frames: u32) {
        let Some(slot) = self.voices.iter_mut().find(|slot| slot.is_none()) else {
            return;
        };
        let frequency = midi_note_frequency(note.note_number());
        *slot = Some(PluginVoice {
            phase: 0.0,
            phase_step: std::f32::consts::TAU * frequency / self.sample_rate_hz,
            velocity: note.velocity(),
            remaining_frames: duration_frames.max(1),
        });
    }
}

fn default_search_paths(format: PluginFormat) -> Box<[PathBuf]> {
    match format {
        PluginFormat::Vst3 => default_vst3_paths(),
        PluginFormat::AudioUnit => default_audio_unit_paths(),
    }
}

fn default_vst3_paths() -> Box<[PathBuf]> {
    let mut paths = Vec::new();
    if cfg!(target_os = "windows") {
        paths.push(PathBuf::from(r"C:\Program Files\Common Files\VST3"));
    } else if cfg!(target_os = "macos") {
        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/VST3"));
        if let Some(home) = home_dir() {
            paths.push(home.join("Library/Audio/Plug-Ins/VST3"));
        }
    } else {
        paths.push(PathBuf::from("/usr/lib/vst3"));
        paths.push(PathBuf::from("/usr/local/lib/vst3"));
        if let Some(home) = home_dir() {
            paths.push(home.join(".vst3"));
        }
    }
    paths.into_boxed_slice()
}

fn default_audio_unit_paths() -> Box<[PathBuf]> {
    let mut paths = Vec::new();
    if cfg!(target_os = "macos") {
        paths.push(PathBuf::from("/Library/Audio/Plug-Ins/Components"));
        if let Some(home) = home_dir() {
            paths.push(home.join("Library/Audio/Plug-Ins/Components"));
        }
    }
    paths.into_boxed_slice()
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn rational_to_frame_offset(value: &Rational, frames_per_cycle: u64) -> Option<u64> {
    if value.numerator() < 0 {
        return None;
    }
    let scaled = value
        .numerator()
        .checked_mul(i128::from(frames_per_cycle))?;
    let offset = scaled / value.denominator();
    u64::try_from(offset).ok()
}

fn note_duration_frames(event: &Event<PluginNote>, frames_per_cycle: u64) -> u32 {
    let start = rational_to_frame_offset(event.part.start(), frames_per_cycle).unwrap_or(0);
    let end = rational_to_frame_offset(event.part.end(), frames_per_cycle).unwrap_or(start + 1);
    let duration = end.saturating_sub(start).max(1);
    u32::try_from(duration).unwrap_or(u32::MAX)
}

fn midi_note_frequency(note_number: u8) -> f32 {
    440.0 * ((f32::from(note_number) - 69.0) / 12.0).exp2()
}
