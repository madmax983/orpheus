use rtrb::{Consumer, Producer, RingBuffer};

/// Commands sent from the UI thread to the audio engine.
#[derive(Clone, Debug, PartialEq)]
pub enum EngineCommand {
    /// Swaps the active pattern after the current cycle completes.
    SwapPattern(String),
    /// Updates the transport tempo in beats per minute.
    SetTempo(f32),
}

pub fn new_command_queue() -> (Producer<EngineCommand>, Consumer<EngineCommand>) {
    RingBuffer::<EngineCommand>::new(64)
}
