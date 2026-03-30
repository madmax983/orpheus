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
    pub(crate) note: u8,
    pub(crate) velocity: u8,
    pub(crate) channel: u8,
    pub(crate) kind: MidiNoteEventKind,
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

pub fn cc_normalized(controller: u8) -> f64 {
    let raw = state().cc_values[controller as usize].load(Ordering::Relaxed);
    f64::from(raw) / 127.0
}

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

pub fn drain_note_events() -> Vec<MidiNoteEvent> {
    state()
        .note_events
        .lock()
        .map_or_else(|_| Vec::new(), |mut queue| queue.drain(..).collect())
}

#[cfg(test)]
pub(crate) fn set_cc_value_for_test(controller: u8, value: u8) {
    state().cc_values[controller as usize].store(value, Ordering::Relaxed);
}
