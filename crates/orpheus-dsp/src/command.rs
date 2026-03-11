use orpheus_pattern::Event;
use rtrb::{Consumer, Producer, RingBuffer};

use crate::sample_bank::SampleBank;

/// Per-event playback parameters resolved before scheduling.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleTrigger {
    token: Box<str>,
    gain: f64,
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

pub fn new_command_queue() -> (Producer<EngineCommand>, Consumer<EngineCommand>) {
    RingBuffer::<EngineCommand>::new(64)
}
