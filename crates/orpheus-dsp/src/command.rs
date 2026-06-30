//! The `command` module defines the data structures used to communicate with the DSP engine.
//!
//! This module provides the message-passing types that flow from the language evaluation
//! thread down to the real-time audio thread. It includes parameter definitions like
//! `SampleTrigger` and system-level operations like `EngineCommand`.

use crate::pedal::PedalProgram;
use std::sync::Arc;

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
    onset_index: Option<u32>,
    slice_start: f64,
    slice_end: f64,
    pan: f64,
    pedal_program: Option<Arc<PedalProgram>>,
}

impl SampleTrigger {
    /// Creates a new `SampleTrigger` with default parameters for the given sample token.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_dsp::SampleTrigger;
    ///
    /// let trigger = SampleTrigger::named("bd");
    /// assert_eq!(trigger.token(), "bd");
    /// assert_eq!(trigger.gain(), 1.0);
    /// ```
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
            onset_index: None,
            slice_start: 0.0,
            slice_end: 1.0,
            pan: 0.0,
            pedal_program: None,
        }
    }

    /// Sets the playback gain multiplier.
    ///
    /// A value of `1.0` is unity gain.
    #[must_use]
    pub const fn with_gain(mut self, gain: f64) -> Self {
        self.gain = gain;
        self
    }

    /// Sets the high-pass filter cutoff frequency in Hertz.
    #[must_use]
    pub const fn with_hpf_cutoff_hz(mut self, cutoff_hz: f64) -> Self {
        self.hpf_cutoff_hz = Some(cutoff_hz);
        self
    }

    /// Sets the low-pass filter cutoff frequency in Hertz.
    #[must_use]
    pub const fn with_lpf_cutoff_hz(mut self, cutoff_hz: f64) -> Self {
        self.lpf_cutoff_hz = Some(cutoff_hz);
        self
    }

    /// Sets the filter resonance (Q factor).
    #[must_use]
    pub const fn with_resonance(mut self, resonance: f64) -> Self {
        self.resonance = resonance;
        self
    }

    /// Sets the saturation drive amount.
    #[must_use]
    pub const fn with_drive(mut self, drive: f64) -> Self {
        self.drive = drive;
        self
    }

    /// Sets the pulse width (duty cycle) for oscillator waveforms.
    #[must_use]
    pub const fn with_pulse_width(mut self, pulse_width: f64) -> Self {
        self.pulse_width = pulse_width;
        self
    }

    /// Sets the playback rate multiplier (pitch shifting).
    ///
    /// A value of `1.0` is normal speed, `2.0` is an octave higher and twice as fast.
    #[must_use]
    pub const fn with_rate(mut self, rate: f64) -> Self {
        self.rate = rate;
        self
    }

    /// Sets the delay effect wet/dry mix level (0.0 to 1.0).
    #[must_use]
    pub const fn with_delay_mix(mut self, mix: f64) -> Self {
        self.delay_mix = mix;
        self
    }

    /// Sets the delay time in fractions of a cycle.
    #[must_use]
    pub const fn with_delay_time(mut self, time: f64) -> Self {
        self.delay_time = time;
        self
    }

    /// Sets the delay feedback amount (0.0 to 1.0).
    #[must_use]
    pub const fn with_delay_feedback(mut self, feedback: f64) -> Self {
        self.delay_feedback = feedback;
        self
    }

    /// Sets the reverb effect wet/dry mix level (0.0 to 1.0).
    #[must_use]
    pub const fn with_reverb_mix(mut self, mix: f64) -> Self {
        self.reverb_mix = mix;
        self
    }

    /// Sets the reverb room size (0.0 to 1.0).
    #[must_use]
    pub const fn with_reverb_room(mut self, room: f64) -> Self {
        self.reverb_room = room;
        self
    }

    /// Sets the reverb high-frequency damping (0.0 to 1.0).
    #[must_use]
    pub const fn with_reverb_damp(mut self, damp: f64) -> Self {
        self.reverb_damp = damp;
        self
    }

    /// Sets the chorus effect wet/dry mix level (0.0 to 1.0).
    #[must_use]
    pub const fn with_chorus_mix(mut self, mix: f64) -> Self {
        self.chorus_mix = mix;
        self
    }

    /// Sets the chorus modulation depth.
    #[must_use]
    pub const fn with_chorus_depth(mut self, depth: f64) -> Self {
        self.chorus_depth = depth;
        self
    }

    /// Sets the chorus modulation rate in Hertz.
    #[must_use]
    pub const fn with_chorus_rate(mut self, rate: f64) -> Self {
        self.chorus_rate = rate;
        self
    }

    /// Sets the compressor wet/dry mix level (0.0 to 1.0).
    #[must_use]
    pub const fn with_compressor_mix(mut self, mix: f64) -> Self {
        self.compressor_mix = mix;
        self
    }

    /// Sets the compressor threshold level.
    #[must_use]
    pub const fn with_compressor_threshold(mut self, threshold: f64) -> Self {
        self.compressor_threshold = threshold;
        self
    }

    /// Sets the compressor ratio (e.g., 4.0 for 4:1).
    #[must_use]
    pub const fn with_compressor_ratio(mut self, ratio: f64) -> Self {
        self.compressor_ratio = ratio;
        self
    }

    /// Selects a specific slice index within the sample using onset detection.
    #[must_use]
    pub const fn with_onset(mut self, onset_index: u32) -> Self {
        self.onset_index = Some(onset_index);
        self
    }

    /// Configures playback to only play a specific fractional slice of the sample (0.0 to 1.0).
    #[must_use]
    pub const fn with_slice(mut self, start: f64, end: f64) -> Self {
        self.slice_start = start;
        self.slice_end = end;
        self
    }

    /// Sets the stereo panning (-1.0 for full left, 1.0 for full right).
    #[must_use]
    pub const fn with_pan(mut self, pan: f64) -> Self {
        self.pan = pan;
        self
    }

    /// The unique name of the sample in the loaded sample bank.
    #[must_use]
    pub fn with_pedal_program(mut self, pedal_program: Arc<PedalProgram>) -> Self {
        self.pedal_program = Some(pedal_program);
        self
    }

    #[doc(hidden)]
    #[must_use]
    pub fn token(&self) -> &str {
        self.token.as_ref()
    }

    /// The overall volume multiplier applied before routing.
    #[must_use]
    pub const fn gain(&self) -> f64 {
        self.gain
    }

    /// The high-pass filter cutoff frequency in Hertz. If `None`, the filter is bypassed.
    #[must_use]
    pub const fn hpf_cutoff_hz(&self) -> Option<f64> {
        self.hpf_cutoff_hz
    }

    /// The low-pass filter cutoff frequency in Hertz. If `None`, the filter is bypassed.
    #[must_use]
    pub const fn lpf_cutoff_hz(&self) -> Option<f64> {
        self.lpf_cutoff_hz
    }

    /// The resonance (Q factor) applied to active high-pass or low-pass filters.
    #[must_use]
    pub const fn resonance(&self) -> f64 {
        self.resonance
    }

    /// The amount of saturation distortion applied to the signal.
    #[must_use]
    pub const fn drive(&self) -> f64 {
        self.drive
    }

    /// The duty cycle used for generated oscillators (e.g., square waves).
    #[must_use]
    pub const fn pulse_width(&self) -> f64 {
        self.pulse_width
    }

    /// The playback speed multiplier, which inherently affects pitch.
    #[must_use]
    pub const fn rate(&self) -> f64 {
        self.rate
    }

    /// The proportion of the signal sent to the local delay effect.
    #[must_use]
    pub const fn delay_mix(&self) -> f64 {
        self.delay_mix
    }

    /// The rhythmic interval used for the delay effect.
    #[must_use]
    pub const fn delay_time(&self) -> f64 {
        self.delay_time
    }

    /// The decay rate of the delay effect.
    #[must_use]
    pub const fn delay_feedback(&self) -> f64 {
        self.delay_feedback
    }

    /// The proportion of the signal sent to the local reverb effect.
    #[must_use]
    pub const fn reverb_mix(&self) -> f64 {
        self.reverb_mix
    }

    /// The simulated spatial size for the local reverb effect.
    #[must_use]
    pub const fn reverb_room(&self) -> f64 {
        self.reverb_room
    }

    /// The high-frequency absorption rate for the local reverb effect.
    #[must_use]
    pub const fn reverb_damp(&self) -> f64 {
        self.reverb_damp
    }

    /// The proportion of the signal sent to the local chorus effect.
    #[must_use]
    pub const fn chorus_mix(&self) -> f64 {
        self.chorus_mix
    }

    /// The depth of modulation for the local chorus effect.
    #[must_use]
    pub const fn chorus_depth(&self) -> f64 {
        self.chorus_depth
    }

    /// The speed of modulation for the local chorus effect in Hertz.
    #[must_use]
    pub const fn chorus_rate(&self) -> f64 {
        self.chorus_rate
    }

    /// The proportion of the signal sent through the local dynamic range compressor.
    #[must_use]
    pub const fn compressor_mix(&self) -> f64 {
        self.compressor_mix
    }

    /// The amplitude boundary where compression begins to reduce gain.
    #[must_use]
    pub const fn compressor_threshold(&self) -> f64 {
        self.compressor_threshold
    }

    /// The steepness of gain reduction applied when the signal exceeds the threshold.
    #[must_use]
    pub const fn compressor_ratio(&self) -> f64 {
        self.compressor_ratio
    }

    /// An explicit onset slice index extracted from the source sample.
    #[must_use]
    pub const fn onset_index(&self) -> Option<u32> {
        self.onset_index
    }

    /// The fractional start position of playback within the full sample.
    #[must_use]
    pub const fn slice_start(&self) -> f64 {
        self.slice_start
    }

    /// The fractional end position of playback within the full sample.
    #[must_use]
    pub const fn slice_end(&self) -> f64 {
        self.slice_end
    }

    /// The spatial balance between the left and right audio channels.
    #[must_use]
    pub const fn pan(&self) -> f64 {
        self.pan
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn pedal_program(&self) -> Option<&Arc<PedalProgram>> {
        self.pedal_program.as_ref()
    }
}

/// A fully resolved unit-cycle pattern ready for audio-thread scheduling.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternUpdate {
    name: Box<str>,
    events: Vec<Event<SampleTrigger>>,
}

impl PatternUpdate {
    /// Creates a new `PatternUpdate` from a sequence of fully evaluated events.
    #[must_use]
    pub fn new(name: impl Into<Box<str>>, events: Vec<Event<SampleTrigger>>) -> Self {
        Self {
            name: name.into(),
            events,
        }
    }

    /// Creates a new `PatternUpdate` that contains no events, representing silence.
    #[must_use]
    pub fn silent(name: impl Into<Box<str>>) -> Self {
        Self::new(name, Vec::new())
    }

    /// The string identifier bound to this pattern, usually derived from the variable name in the environment.
    #[doc(hidden)]
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// The chronologically sorted events that constitute exactly one cycle of playback.
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
    /// Updates the analog-voice reference frequency in Hertz.
    ///
    /// Applied immediately on the audio thread; affects every new analog voice
    /// allocation. The default is [`crate::DEFAULT_ANALOG_BASE_FREQUENCY_HZ`]
    /// (220 Hz / A3). Use 432.0 for A4 = 432 Hz tuning, for example.
    SetReferenceFrequency(f32),
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
#[must_use]
pub fn new_command_queue() -> (Producer<EngineCommand>, Consumer<EngineCommand>) {
    RingBuffer::<EngineCommand>::new(64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use orpheus_pattern::TimeSpan;

    fn assert_trigger_core_defaults(trigger: &SampleTrigger) {
        assert_eq!(trigger.token(), "bd");
        assert!((trigger.gain() - 1.0).abs() < f64::EPSILON);
        assert!((trigger.pan() - 0.0).abs() < f64::EPSILON);
        assert!((trigger.rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(trigger.onset_index(), None);
        assert!((trigger.slice_start() - 0.0).abs() < f64::EPSILON);
        assert!((trigger.slice_end() - 1.0).abs() < f64::EPSILON);
        assert!(trigger.pedal_program().is_none());
    }

    fn assert_trigger_filter_defaults(trigger: &SampleTrigger) {
        assert_eq!(trigger.hpf_cutoff_hz(), None);
        assert_eq!(trigger.lpf_cutoff_hz(), None);
        assert!((trigger.resonance() - 0.2).abs() < f64::EPSILON);
        assert!((trigger.drive() - 1.0).abs() < f64::EPSILON);
        assert!((trigger.pulse_width() - 0.5).abs() < f64::EPSILON);
    }

    fn assert_trigger_fx_defaults(trigger: &SampleTrigger) {
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
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn sample_trigger_named_initializes_with_default_values() {
        let trigger = SampleTrigger::named("bd");
        assert_trigger_core_defaults(&trigger);
        assert_trigger_filter_defaults(&trigger);
        assert_trigger_fx_defaults(&trigger);
    }

    fn assert_trigger_core_updates(trigger: &SampleTrigger, pedal_program: &Arc<PedalProgram>) {
        assert_eq!(trigger.token(), "sn");
        assert!((trigger.gain() - 0.8).abs() < f64::EPSILON);
        assert!((trigger.pan() - 0.3).abs() < f64::EPSILON);
        assert!((trigger.rate() - 1.5).abs() < f64::EPSILON);
        assert_eq!(trigger.onset_index(), Some(3));
        assert!((trigger.slice_start() - 0.2).abs() < f64::EPSILON);
        assert!((trigger.slice_end() - 0.8).abs() < f64::EPSILON);
        assert!(Arc::ptr_eq(
            trigger
                .pedal_program()
                .expect("sample trigger should expose the pedal program"),
            pedal_program
        ));
    }

    fn assert_trigger_filter_updates(trigger: &SampleTrigger) {
        assert_eq!(trigger.hpf_cutoff_hz(), Some(500.0));
        assert_eq!(trigger.lpf_cutoff_hz(), Some(12000.0));
        assert!((trigger.resonance() - 0.35).abs() < f64::EPSILON);
        assert!((trigger.drive() - 1.25).abs() < f64::EPSILON);
        assert!((trigger.pulse_width() - 0.42).abs() < f64::EPSILON);
    }

    fn assert_trigger_fx_updates(trigger: &SampleTrigger) {
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
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn sample_trigger_builder_methods_update_fields() {
        let pedal_program = Arc::new(PedalProgram::new("graph { input |> output }", "result"));
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
            .with_onset(3)
            .with_slice(0.2, 0.8)
            .with_pan(0.3)
            .with_pedal_program(pedal_program.clone());

        assert_trigger_core_updates(&trigger, &pedal_program);
        assert_trigger_filter_updates(&trigger);
        assert_trigger_fx_updates(&trigger);
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
