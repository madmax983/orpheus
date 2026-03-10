use orpheus_pattern::Event;
use rtrb::{Consumer, Producer, RingBuffer};

use crate::sample_bank::SampleBank;

/// A fully resolved unit-cycle pattern ready for audio-thread scheduling.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternUpdate {
    name: Box<str>,
    events: Vec<Event<Box<str>>>,
}

impl PatternUpdate {
    #[must_use]
    pub fn new(name: impl Into<Box<str>>, events: Vec<Event<Box<str>>>) -> Self {
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
    pub fn events(&self) -> &[Event<Box<str>>] {
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
