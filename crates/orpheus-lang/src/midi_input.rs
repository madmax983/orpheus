#![allow(clippy::float_cmp)]
//! Shared MIDI input state for control and note events.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]

/// Represents the kind of a MIDI note event (either note on or note off).
pub enum MidiNoteEventKind {
    /// A MIDI note on event.
    On,
    /// A MIDI note off event.
    Off,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Represents a parsed MIDI note event containing pitch, velocity, and channel information.
///
/// Used by the runtime to inject external MIDI keyboard events into the evaluation context.
pub struct MidiNoteEvent {
    pub note: u8,
    pub velocity: u8,
    pub channel: u8,
    pub kind: MidiNoteEventKind,
}

struct MidiInputSharedState {
    cc_values: [AtomicU8; 128],
    note_events: Mutex<VecDeque<MidiNoteEvent>>,
}

impl MidiInputSharedState {
    fn new() -> Self {
        Self {
            cc_values: std::array::from_fn(|_| AtomicU8::new(0)),
            note_events: Mutex::new(VecDeque::new()),
        }
    }
}

fn state() -> &'static MidiInputSharedState {
    static STATE: OnceLock<MidiInputSharedState> = OnceLock::new();
    STATE.get_or_init(MidiInputSharedState::new)
}

#[cfg(test)]
pub fn reset_state_for_test() {
    // Cannot easily reset OnceLock, but we can clear the internal data
    if let Ok(mut queue) = state().note_events.lock() {
        queue.clear();
    }
    for val in &state().cc_values {
        val.store(0, Ordering::Relaxed);
    }
}

/// Retrieves the normalized value of a specific MIDI continuous controller (CC).
///
/// Returns a value between `0.0` and `1.0` representing the current state of the given controller.
/// Used for mapping hardware knobs and sliders to pattern parameters.
///
/// # Examples
///
/// ```
/// use orpheus_lang::midi_input;
///
/// let cc_val = midi_input::cc_normalized(1); // Mod wheel
/// assert!(cc_val >= 0.0 && cc_val <= 1.0);
/// ```
pub fn cc_normalized(controller: u8) -> f64 {
    state()
        .cc_values
        .get(controller as usize)
        .map_or(0.0, |atomic_val| {
            let raw = atomic_val.load(Ordering::Relaxed);
            f64::from(raw) / 127.0
        })
}

/// Updates the global MIDI input state from a raw MIDI message.
///
/// This function parses 3-byte MIDI messages (Note On, Note Off, Control Change)
/// and updates the shared state accessed by `cc_normalized` and `drain_note_events`.
///
/// # Panics
///
/// Panics if the internal shared state mutex is poisoned.
///
/// # Examples
///
/// ```
/// use orpheus_lang::midi_input;
///
/// // Simulate a Note On message: channel 1, note 60, velocity 100
/// midi_input::update_from_message(&[0x90, 60, 100]);
/// ```
pub fn update_from_message(message: &[u8]) {
    if message.is_empty() {
        return;
    }
    let status = message[0];
    let kind = status & 0xF0;
    let channel = status & 0x0F;
    match kind {
        0x80 | 0x90 if message.len() >= 3 => {
            let note = message[1];
            let velocity = message[2];
            let event_kind = if kind == 0x80 || velocity == 0 {
                MidiNoteEventKind::Off
            } else {
                MidiNoteEventKind::On
            };
            if let Ok(mut queue) = state().note_events.lock() {
                queue.push_back(MidiNoteEvent {
                    note,
                    velocity,
                    channel,
                    kind: event_kind,
                });
                while queue.len() > 1024 {
                    let _ = queue.pop_front();
                }
            }
        }
        0xB0 if message.len() >= 3 => {
            let controller = message[1];
            let value = message[2];
            if controller < 128 {
                state().cc_values[controller as usize].store(value, Ordering::Relaxed);
            }
        }
        _ => {}
    }
}

/// Drains all accumulated MIDI note events since the last call.
///
/// This is used by the language runtime engine to flush new notes per-cycle
/// and convert them into live performance events.
///
/// # Panics
///
/// Panics if the internal shared state mutex is poisoned.
///
/// # Examples
///
/// ```
/// use orpheus_lang::midi_input;
///
/// let notes = midi_input::drain_note_events();
/// for note in notes {
///     println!("Received note: {}", note.note);
/// }
/// ```
pub fn drain_note_events() -> Vec<MidiNoteEvent> {
    state()
        .note_events
        .lock()
        .map_or_else(|_| Vec::new(), |mut queue| queue.drain(..).collect())
}

#[cfg(test)]
pub fn set_cc_value_for_test(controller: u8, value: u8) {
    if let Some(atomic_val) = state().cc_values.get(controller as usize) {
        atomic_val.store(value, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 👺 Havoc: Tests that rapid concurrent MIDI messages across multiple threads
    /// do not cause deadlocks or corrupt the underlying message queues or CC atomic state.
    #[test]
    fn test_havoc_midi_input_concurrency() {
        loom::model(|| {
            // We just need a few operations under loom to verify there are no data races or deadlocks.
            // Reset state first to isolate runs.
            reset_state_for_test();

            let t1 = loom::thread::spawn(|| {
                update_from_message(&[0x90, 60, 100]); // Note On
                update_from_message(&[0xB0, 10, 127]); // CC
            });

            let t2 = loom::thread::spawn(|| {
                drain_note_events();
                cc_normalized(10);
            });

            t1.join().unwrap();
            t2.join().unwrap();
        });
    }

    #[test]
    fn test_cc_normalized_out_of_bounds() {
        // Should not panic and return 0.0
        let val = cc_normalized(128);
        assert_eq!(val, 0.0);
    }

    #[test]
    fn test_set_cc_value_for_test_out_of_bounds() {
        // Should not panic
        set_cc_value_for_test(128, 64);
    }

    /// 👺 Havoc: Tests that injecting malformed or garbage byte sequences into the
    /// MIDI message parser correctly ignores them without panicking via out-of-bounds indexing.
    #[test]
    fn test_havoc_update_from_message_fuzz() {
        // Havoc: Inject garbage bytes to simulate invalid MIDI messages.
        // It shouldn't panic on array indexing.
        update_from_message(&[0xB0, 255, 255]); // OOB controller CC
        update_from_message(&[0x90, 255, 255]); // High note/velocity
        update_from_message(&[0xFF, 0xFF, 0xFF, 0xFF]); // Random bytes
        update_from_message(&[]); // Empty
        update_from_message(&[0xB0]); // Incomplete
    }
}
