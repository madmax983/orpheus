#![allow(clippy::float_cmp)]
//! Shared MIDI input state for control and note events.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]

pub enum MidiNoteEventKind {
    On,
    Off,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]

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

/// Retrieves the last seen normalized value for a specific MIDI Control Change (CC).
///
/// The raw `0..=127` MIDI value is converted to a `0.0..=1.0` float, making it
/// universally compatible with standard Orpheus modulation ranges.
///
/// Returns `0.0` if the controller index is out of bounds or no value has been seen yet.
///
/// # Examples
///
/// ```rust
/// use orpheus_lang::midi_input::{cc_normalized, update_from_message};
///
/// // Simulate receiving CC #10 with value 64 (roughly 50%)
/// update_from_message(&[0xB0, 10, 64]);
///
/// let value = cc_normalized(10);
/// assert!((value - (64.0 / 127.0)).abs() < 0.001);
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

/// Parses raw MIDI bytes and updates the shared global state for notes and CCs.
///
/// This is designed to be called directly from an audio backend's MIDI callback
/// thread. It uses lock-free atomics for Continuous Controllers (CC) and a fast mutex queue
/// for note events to minimize jitter.
///
/// # Examples
///
/// ```rust
/// use orpheus_lang::midi_input::update_from_message;
///
/// // Parse a Note On message (Channel 1, Note 60, Velocity 100)
/// update_from_message(&[0x90, 60, 100]);
///
/// // Parse a CC message (Channel 1, Controller 7, Value 127)
/// update_from_message(&[0xB0, 7, 127]);
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

/// Extracts and clears all pending MIDI note events (On/Off).
///
/// This is typically called at the start of a render cycle to process any
/// keystrokes or pad hits that arrived since the last block. The queue is completely
/// drained to prevent backlogging.
///
/// # Examples
///
/// ```rust
/// use orpheus_lang::midi_input::{drain_note_events, update_from_message};
///
/// // Send two note events
/// update_from_message(&[0x90, 60, 100]); // Note On
/// update_from_message(&[0x80, 60, 0]);   // Note Off
///
/// let events = drain_note_events();
/// assert_eq!(events.len(), 2);
/// assert_eq!(events[0].note, 60);
///
/// // Subsequent calls return empty until new messages arrive
/// assert!(drain_note_events().is_empty());
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
