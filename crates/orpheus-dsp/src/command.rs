use orpheus_pattern::Event;
use rtrb::{Consumer, Producer, RingBuffer};

use crate::sample_bank::SampleBank;

/// Per-event playback parameters resolved before scheduling.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleTrigger {
    token: Box<str>,
    gain: f64,
    hpf_cutoff_hz: Option<f64>,
    lpf_cutoff_hz: Option<f64>,
    rate: f64,
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
            rate: 1.0,
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
    pub const fn with_rate(mut self, rate: f64) -> Self {
        self.rate = rate;
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
    pub const fn rate(&self) -> f64 {
        self.rate
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
/// ```
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
        assert!((trigger.rate() - 1.0).abs() < f64::EPSILON);
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
            .with_rate(1.5)
            .with_slice(0.2, 0.8)
            .with_pan(0.3);

        assert_eq!(trigger.token(), "sn");
        assert!((trigger.gain() - 0.8).abs() < f64::EPSILON);
        assert_eq!(trigger.hpf_cutoff_hz(), Some(500.0));
        assert_eq!(trigger.lpf_cutoff_hz(), Some(12000.0));
        assert!((trigger.rate() - 1.5).abs() < f64::EPSILON);
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
