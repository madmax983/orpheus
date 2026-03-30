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
    /// Creates a new `SampleTrigger` with the specified token and default parameters.
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

    /// Builder method to override the trigger's gain.
    #[must_use]
    pub const fn with_gain(mut self, gain: f64) -> Self {
        self.gain = gain;
        self
    }

    /// Builder method to override the trigger's high-pass filter cutoff in Hertz.
    #[must_use]
    pub const fn with_hpf_cutoff_hz(mut self, cutoff_hz: f64) -> Self {
        self.hpf_cutoff_hz = Some(cutoff_hz);
        self
    }

    /// Builder method to override the trigger's low-pass filter cutoff in Hertz.
    #[must_use]
    pub const fn with_lpf_cutoff_hz(mut self, cutoff_hz: f64) -> Self {
        self.lpf_cutoff_hz = Some(cutoff_hz);
        self
    }

    /// Builder method to override the trigger's playback rate.
    #[must_use]
    pub const fn with_rate(mut self, rate: f64) -> Self {
        self.rate = rate;
        self
    }

    /// Builder method to override the trigger's playback slice boundaries.
    #[must_use]
    pub const fn with_slice(mut self, start: f64, end: f64) -> Self {
        self.slice_start = start;
        self.slice_end = end;
        self
    }

    /// Builder method to override the trigger's stereo panning position.
    #[must_use]
    pub const fn with_pan(mut self, pan: f64) -> Self {
        self.pan = pan;
        self
    }

    /// The string identifier linking this event back to a loaded file in the `SampleBank`.
    #[must_use]
    pub fn token(&self) -> &str {
        self.token.as_ref()
    }

    /// The linear amplitude multiplier scaling the raw sample frames.
    #[must_use]
    pub const fn gain(&self) -> f64 {
        self.gain
    }

    /// The cutoff frequency where the high-pass filter begins rolling off bass frequencies.
    #[must_use]
    pub const fn hpf_cutoff_hz(&self) -> Option<f64> {
        self.hpf_cutoff_hz
    }

    /// The cutoff frequency where the low-pass filter begins rolling off treble frequencies.
    #[must_use]
    pub const fn lpf_cutoff_hz(&self) -> Option<f64> {
        self.lpf_cutoff_hz
    }

    /// The playback speed modifier. For instance, `2.0` plays the sample twice as fast,
    /// pitching it up an octave.
    #[must_use]
    pub const fn rate(&self) -> f64 {
        self.rate
    }

    /// The normalized `[0, 1]` fraction indicating where playback should begin within the sample.
    #[must_use]
    pub const fn slice_start(&self) -> f64 {
        self.slice_start
    }

    /// The normalized `[0, 1]` fraction indicating where playback should cease within the sample.
    #[must_use]
    pub const fn slice_end(&self) -> f64 {
        self.slice_end
    }

    /// The stereophonic pan applied to the sample mix, ranging from `-1.0` (hard left) to `1.0` (hard right).
    #[must_use]
    pub const fn pan(&self) -> f64 {
        self.pan
    }
}

/// Represents an atomic pattern replacement sent from the REPL to the audio thread.
///
/// To maintain rhythmic integrity, Orpheus defers actual sequence swaps until the playhead
/// crosses a cycle boundary (e.g., the "one" of a measure). A `PatternUpdate` packages the fully
/// evaluated, explicit `SampleTrigger` events for one complete unit cycle.
#[derive(Clone, Debug, PartialEq)]
pub struct PatternUpdate {
    name: Box<str>,
    events: Vec<Event<SampleTrigger>>,
}

impl PatternUpdate {
    /// Prepares a new pattern payload bound for the DSP ring buffer.
    #[must_use]
    pub fn new(name: impl Into<Box<str>>, events: Vec<Event<SampleTrigger>>) -> Self {
        Self {
            name: name.into(),
            events,
        }
    }

    /// Creates a dummy pattern containing no events, causing the DSP engine to effectively stop output.
    #[must_use]
    pub fn silent(name: impl Into<Box<str>>) -> Self {
        Self::new(name, Vec::new())
    }

    /// The name assigned to the evaluated module, used extensively by the TUI rendering systems.
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// The immutable slice of scheduled playback commands queued for rendering over the next cycle.
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
