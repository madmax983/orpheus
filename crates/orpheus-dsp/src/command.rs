//! The `command` module defines the data structures used to communicate with the DSP engine.
//!
//! This module provides the message-passing types that flow from the language evaluation
//! thread down to the real-time audio thread. It includes parameter definitions like
//! `SampleTrigger` and system-level operations like `EngineCommand`.

use orpheus_pattern::Event;
use rtrb::{Consumer, Producer, RingBuffer};

use crate::routing::RoutingSnapshot;
use crate::sample_bank::SampleBank;

/// Per-event playback parameters resolved before scheduling.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleTrigger {
    token: Box<str>,
    gain: f64,
    hpf_cutoff_hz: Option<f64>,
    lpf_cutoff_hz: Option<f64>,
    resonance: f64,
    drive: f64,
    pulse_width: f64,
    rate: f64,
    delay_mix: f64,
    delay_time: f64,
    delay_feedback: f64,
    reverb_mix: f64,
    reverb_room: f64,
    reverb_damp: f64,
    chorus_mix: f64,
    chorus_depth: f64,
    chorus_rate: f64,
    compressor_mix: f64,
    compressor_threshold: f64,
    compressor_ratio: f64,
    slice_start: f64,
    slice_end: f64,
    pan: f64,
}

impl SampleTrigger {
    #[must_use]
    pub fn named(token: impl Into<Box<str>>) -> Self {
        Self {
            token: token.into(),
            gain: 1.0,
            hpf_cutoff_hz: None,
            lpf_cutoff_hz: None,
            resonance: 0.2,
            drive: 1.0,
            pulse_width: 0.5,
            rate: 1.0,
            delay_mix: 0.0,
            delay_time: 0.125,
            delay_feedback: 0.35,
            reverb_mix: 0.0,
            reverb_room: 0.75,
            reverb_damp: 0.35,
            chorus_mix: 0.0,
            chorus_depth: 0.4,
            chorus_rate: 0.5,
            compressor_mix: 0.0,
            compressor_threshold: 0.5,
            compressor_ratio: 4.0,
            slice_start: 0.0,
            slice_end: 1.0,
            pan: 0.0,
        }
    }

    #[must_use]
    pub const fn with_gain(mut self, gain: f64) -> Self {
        self.gain = gain;
        self
    }

    #[must_use]
    pub const fn with_hpf_cutoff_hz(mut self, cutoff_hz: f64) -> Self {
        self.hpf_cutoff_hz = Some(cutoff_hz);
        self
    }

    #[must_use]
    pub const fn with_lpf_cutoff_hz(mut self, cutoff_hz: f64) -> Self {
        self.lpf_cutoff_hz = Some(cutoff_hz);
        self
    }

    #[must_use]
    pub const fn with_resonance(mut self, resonance: f64) -> Self {
        self.resonance = resonance;
        self
    }

    #[must_use]
    pub const fn with_drive(mut self, drive: f64) -> Self {
        self.drive = drive;
        self
    }

    #[must_use]
    pub const fn with_pulse_width(mut self, pulse_width: f64) -> Self {
        self.pulse_width = pulse_width;
        self
    }

    #[must_use]
    pub const fn with_rate(mut self, rate: f64) -> Self {
        self.rate = rate;
        self
    }

    #[must_use]
    pub const fn with_delay_mix(mut self, mix: f64) -> Self {
        self.delay_mix = mix;
        self
    }

    #[must_use]
    pub const fn with_delay_time(mut self, time: f64) -> Self {
        self.delay_time = time;
        self
    }

    #[must_use]
    pub const fn with_delay_feedback(mut self, feedback: f64) -> Self {
        self.delay_feedback = feedback;
        self
    }

    #[must_use]
    pub const fn with_reverb_mix(mut self, mix: f64) -> Self {
        self.reverb_mix = mix;
        self
    }

    #[must_use]
    pub const fn with_reverb_room(mut self, room: f64) -> Self {
        self.reverb_room = room;
        self
    }

    #[must_use]
    pub const fn with_reverb_damp(mut self, damp: f64) -> Self {
        self.reverb_damp = damp;
        self
    }

    #[must_use]
    pub const fn with_chorus_mix(mut self, mix: f64) -> Self {
        self.chorus_mix = mix;
        self
    }

    #[must_use]
    pub const fn with_chorus_depth(mut self, depth: f64) -> Self {
        self.chorus_depth = depth;
        self
    }

    #[must_use]
    pub const fn with_chorus_rate(mut self, rate: f64) -> Self {
        self.chorus_rate = rate;
        self
    }

    #[must_use]
    pub const fn with_compressor_mix(mut self, mix: f64) -> Self {
        self.compressor_mix = mix;
        self
    }

    #[must_use]
    pub const fn with_compressor_threshold(mut self, threshold: f64) -> Self {
        self.compressor_threshold = threshold;
        self
    }

    #[must_use]
    pub const fn with_compressor_ratio(mut self, ratio: f64) -> Self {
        self.compressor_ratio = ratio;
        self
    }

    #[must_use]
    pub const fn with_slice(mut self, start: f64, end: f64) -> Self {
        self.slice_start = start;
        self.slice_end = end;
        self
    }

    #[must_use]
    pub const fn with_pan(mut self, pan: f64) -> Self {
        self.pan = pan;
        self
    }

    #[must_use]
    pub fn token(&self) -> &str {
        self.token.as_ref()
    }

    #[must_use]
    pub const fn gain(&self) -> f64 {
        self.gain
    }

    #[must_use]
    pub const fn hpf_cutoff_hz(&self) -> Option<f64> {
        self.hpf_cutoff_hz
    }

    #[must_use]
    pub const fn lpf_cutoff_hz(&self) -> Option<f64> {
        self.lpf_cutoff_hz
    }

    #[must_use]
    pub const fn resonance(&self) -> f64 {
        self.resonance
    }

    #[must_use]
    pub const fn drive(&self) -> f64 {
        self.drive
    }

    #[must_use]
    pub const fn pulse_width(&self) -> f64 {
        self.pulse_width
    }

    #[must_use]
    pub const fn rate(&self) -> f64 {
        self.rate
    }

    #[must_use]
    pub const fn delay_mix(&self) -> f64 {
        self.delay_mix
    }

    #[must_use]
    pub const fn delay_time(&self) -> f64 {
        self.delay_time
    }

    #[must_use]
    pub const fn delay_feedback(&self) -> f64 {
        self.delay_feedback
    }

    #[must_use]
    pub const fn reverb_mix(&self) -> f64 {
        self.reverb_mix
    }

    #[must_use]
    pub const fn reverb_room(&self) -> f64 {
        self.reverb_room
    }

    #[must_use]
    pub const fn reverb_damp(&self) -> f64 {
        self.reverb_damp
    }

    #[must_use]
    pub const fn chorus_mix(&self) -> f64 {
        self.chorus_mix
    }

    #[must_use]
    pub const fn chorus_depth(&self) -> f64 {
        self.chorus_depth
    }

    #[must_use]
    pub const fn chorus_rate(&self) -> f64 {
        self.chorus_rate
    }

    #[must_use]
    pub const fn compressor_mix(&self) -> f64 {
        self.compressor_mix
    }

    #[must_use]
    pub const fn compressor_threshold(&self) -> f64 {
        self.compressor_threshold
    }

    #[must_use]
    pub const fn compressor_ratio(&self) -> f64 {
        self.compressor_ratio
    }

    #[must_use]
    pub const fn slice_start(&self) -> f64 {
        self.slice_start
    }

    #[must_use]
    pub const fn slice_end(&self) -> f64 {
        self.slice_end
    }

    #[must_use]
    pub const fn pan(&self) -> f64 {
        self.pan
    }
}

/// A fully resolved unit-cycle pattern ready for audio-thread scheduling.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternUpdate {
    name: Box<str>,
    events: Vec<Event<SampleTrigger>>,
}

impl PatternUpdate {
    #[must_use]
    pub fn new(name: impl Into<Box<str>>, events: Vec<Event<SampleTrigger>>) -> Self {
        Self {
            name: name.into(),
            events,
        }
    }

    #[must_use]
    pub fn silent(name: impl Into<Box<str>>) -> Self {
        Self::new(name, Vec::new())
    }

    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    #[must_use]
    pub fn events(&self) -> &[Event<SampleTrigger>] {
        &self.events
    }
}

/// Commands sent from the UI thread to the audio engine.
#[derive(Clone, Debug, PartialEq)]
pub enum EngineCommand {
    /// Swaps the active pattern after the current cycle completes.
    SwapPattern(String),
    /// Swaps in a concrete unit-cycle pattern at the next cycle boundary.
    LoadPattern(PatternUpdate),
    /// Swaps in a validated routing snapshot at the next cycle boundary.
    SwapRoutingSnapshot(RoutingSnapshot),
    /// Replaces the sample bank at the next cycle boundary.
    ReplaceSampleBank(SampleBank),
    /// Updates the transport tempo in beats per minute.
    SetTempo(f32),
    /// Starts transport playback from the current rewound position.
    PlayTransport,
    /// Stops transport playback, silencing output and rewinding to the start.
    StopTransport,
}

/// Creates a new lock-free ring buffer queue for safely sending commands to the audio thread.
///
/// This avoids lock contention between the UI thread and the real-time audio thread.
///
/// ## Examples
///
/// ```ignore
/// use orpheus_dsp::{new_command_queue, EngineCommand};
///
/// let (mut producer, mut consumer) = new_command_queue();
/// producer.push(EngineCommand::PlayTransport).unwrap();
///
/// assert!(matches!(consumer.pop().unwrap(), EngineCommand::PlayTransport));
/// ```
pub fn new_command_queue() -> (Producer<EngineCommand>, Consumer<EngineCommand>) {
    RingBuffer::<EngineCommand>::new(64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use orpheus_pattern::TimeSpan;

    #[test]
    #[allow(clippy::float_cmp)]
    fn sample_trigger_named_initializes_with_default_values() {
        let trigger = SampleTrigger::named("bd");
        assert_eq!(trigger.token(), "bd");
        assert!((trigger.gain() - 1.0).abs() < f64::EPSILON);
        assert_eq!(trigger.hpf_cutoff_hz(), None);
        assert_eq!(trigger.lpf_cutoff_hz(), None);
        assert!((trigger.resonance() - 0.2).abs() < f64::EPSILON);
        assert!((trigger.drive() - 1.0).abs() < f64::EPSILON);
        assert!((trigger.pulse_width() - 0.5).abs() < f64::EPSILON);
        assert!((trigger.rate() - 1.0).abs() < f64::EPSILON);
        assert!((trigger.delay_mix() - 0.0).abs() < f64::EPSILON);
        assert!((trigger.delay_time() - 0.125).abs() < f64::EPSILON);
        assert!((trigger.delay_feedback() - 0.35).abs() < f64::EPSILON);
        assert!((trigger.reverb_mix() - 0.0).abs() < f64::EPSILON);
        assert!((trigger.reverb_room() - 0.75).abs() < f64::EPSILON);
        assert!((trigger.reverb_damp() - 0.35).abs() < f64::EPSILON);
        assert!((trigger.chorus_mix() - 0.0).abs() < f64::EPSILON);
        assert!((trigger.chorus_depth() - 0.4).abs() < f64::EPSILON);
        assert!((trigger.chorus_rate() - 0.5).abs() < f64::EPSILON);
        assert!((trigger.compressor_mix() - 0.0).abs() < f64::EPSILON);
        assert!((trigger.compressor_threshold() - 0.5).abs() < f64::EPSILON);
        assert!((trigger.compressor_ratio() - 4.0).abs() < f64::EPSILON);
        assert!((trigger.slice_start() - 0.0).abs() < f64::EPSILON);
        assert!((trigger.slice_end() - 1.0).abs() < f64::EPSILON);
        assert!((trigger.pan() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn sample_trigger_builder_methods_update_fields() {
        let trigger = SampleTrigger::named("sn")
            .with_gain(0.8)
            .with_hpf_cutoff_hz(500.0)
            .with_lpf_cutoff_hz(12000.0)
            .with_resonance(0.35)
            .with_drive(1.25)
            .with_pulse_width(0.42)
            .with_rate(1.5)
            .with_delay_mix(0.3)
            .with_delay_time(0.125)
            .with_delay_feedback(0.45)
            .with_reverb_mix(0.2)
            .with_reverb_room(0.8)
            .with_reverb_damp(0.25)
            .with_chorus_mix(0.4)
            .with_chorus_depth(0.7)
            .with_chorus_rate(0.6)
            .with_compressor_mix(0.75)
            .with_compressor_threshold(0.3)
            .with_compressor_ratio(6.0)
            .with_slice(0.2, 0.8)
            .with_pan(0.3);

        assert_eq!(trigger.token(), "sn");
        assert!((trigger.gain() - 0.8).abs() < f64::EPSILON);
        assert_eq!(trigger.hpf_cutoff_hz(), Some(500.0));
        assert_eq!(trigger.lpf_cutoff_hz(), Some(12000.0));
        assert!((trigger.resonance() - 0.35).abs() < f64::EPSILON);
        assert!((trigger.drive() - 1.25).abs() < f64::EPSILON);
        assert!((trigger.pulse_width() - 0.42).abs() < f64::EPSILON);
        assert!((trigger.rate() - 1.5).abs() < f64::EPSILON);
        assert!((trigger.delay_mix() - 0.3).abs() < f64::EPSILON);
        assert!((trigger.delay_time() - 0.125).abs() < f64::EPSILON);
        assert!((trigger.delay_feedback() - 0.45).abs() < f64::EPSILON);
        assert!((trigger.reverb_mix() - 0.2).abs() < f64::EPSILON);
        assert!((trigger.reverb_room() - 0.8).abs() < f64::EPSILON);
        assert!((trigger.reverb_damp() - 0.25).abs() < f64::EPSILON);
        assert!((trigger.chorus_mix() - 0.4).abs() < f64::EPSILON);
        assert!((trigger.chorus_depth() - 0.7).abs() < f64::EPSILON);
        assert!((trigger.chorus_rate() - 0.6).abs() < f64::EPSILON);
        assert!((trigger.compressor_mix() - 0.75).abs() < f64::EPSILON);
        assert!((trigger.compressor_threshold() - 0.3).abs() < f64::EPSILON);
        assert!((trigger.compressor_ratio() - 6.0).abs() < f64::EPSILON);
        assert!((trigger.slice_start() - 0.2).abs() < f64::EPSILON);
        assert!((trigger.slice_end() - 0.8).abs() < f64::EPSILON);
        assert!((trigger.pan() - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn pattern_update_new_stores_name_and_events() {
        let events = vec![Event {
            whole: None,
            part: TimeSpan::unit(),
            value: SampleTrigger::named("cp"),
        }];

        let update = PatternUpdate::new("my_pattern", events.clone());
        assert_eq!(update.name(), "my_pattern");
        assert_eq!(update.events(), events.as_slice());
    }

    #[test]
    fn pattern_update_silent_creates_empty_events_list() {
        let update = PatternUpdate::silent("quiet");
        assert_eq!(update.name(), "quiet");
        assert!(update.events().is_empty());
    }
}
