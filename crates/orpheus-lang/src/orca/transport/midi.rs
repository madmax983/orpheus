//! MIDI byte assembly and output sinks.
//!
//! Byte layouts transcribe the reference io layer exactly (`io/midi.js`
//! `trigger()`, `io/cc.js` `run()`): note-on `0x90 + channel`, note-off
//! `0x80 + channel`, both carrying the velocity byte
//! `floor(velocity / 16 * 127)`; control change `0xB0 + channel` with the
//! knob offset by [`super::DEFAULT_CC_OFFSET`]; pitch bend `0xE0 + channel`
//! with raw (already 0-127-scaled) lsb/msb bytes.
//!
//! Real output goes through [`MidirSink`]; tests use [`RecordingMidiSink`],
//! so nothing in this crate's test suite touches hardware.

use std::sync::{Arc, Mutex};

use midir::{MidiOutput, MidiOutputConnection};

use super::TransportError;

/// A destination for raw MIDI bytes.
///
/// Implemented by [`MidirSink`] (a real device port) and
/// [`RecordingMidiSink`] (a test mock); the dispatcher only ever sees this
/// trait, so all note/CC/pitch-bend behavior is testable without hardware.
pub trait MidiSink: Send {
    /// Sends one complete MIDI message.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError`] when the underlying output rejects the
    /// message; the dispatcher reports the failure and keeps running.
    fn send(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
}

/// A [`MidiSink`] that records every message for later inspection. Clones
/// share the same buffer, so a test can keep one clone and hand the other
/// to a dispatcher or worker thread.
#[derive(Clone, Debug, Default)]
pub struct RecordingMidiSink {
    messages: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl RecordingMidiSink {
    /// An empty recording sink.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A snapshot of every message sent so far, in send order.
    ///
    /// # Panics
    ///
    /// Panics if a previous holder of the buffer panicked mid-send
    /// (poisoned lock) — impossible in practice, as sends never panic.
    #[must_use]
    pub fn messages(&self) -> Vec<Vec<u8>> {
        self.messages.lock().expect("recording lock").clone()
    }
}

impl MidiSink for RecordingMidiSink {
    fn send(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.messages
            .lock()
            .expect("recording lock")
            .push(bytes.to_vec());
        Ok(())
    }
}

/// A [`MidiSink`] backed by a real `midir` output connection.
pub struct MidirSink {
    connection: MidiOutputConnection,
}

impl std::fmt::Debug for MidirSink {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("MidirSink").finish_non_exhaustive()
    }
}

impl MidiSink for MidirSink {
    fn send(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.connection
            .send(bytes)
            .map_err(|error| TransportError::MidiSend(error.to_string()))
    }
}

/// The names of the available MIDI output ports, sorted.
///
/// # Errors
///
/// Returns [`TransportError::MidiInit`] when the MIDI subsystem is
/// unavailable.
pub fn midi_output_names() -> Result<Vec<String>, TransportError> {
    let output = MidiOutput::new("orpheus-orca")
        .map_err(|error| TransportError::MidiInit(error.to_string()))?;
    let mut names = output
        .ports()
        .iter()
        .filter_map(|port| output.port_name(port).ok())
        .collect::<Vec<_>>();
    names.sort_unstable();
    Ok(names)
}

/// Connects to the MIDI output port with exactly `port_name` (the same
/// exact-name convention as the session's `:midi connect`).
///
/// # Errors
///
/// Returns [`TransportError::MidiInit`] when the subsystem is unavailable,
/// [`TransportError::MidiPortNotFound`] when no port matches, and
/// [`TransportError::MidiConnect`] when the connection fails.
pub fn connect_midi_output(port_name: &str) -> Result<MidirSink, TransportError> {
    let output = MidiOutput::new("orpheus-orca")
        .map_err(|error| TransportError::MidiInit(error.to_string()))?;
    let port = output
        .ports()
        .into_iter()
        .find(|candidate| {
            output
                .port_name(candidate)
                .is_ok_and(|name| name == port_name)
        })
        .ok_or_else(|| TransportError::MidiPortNotFound(port_name.to_owned()))?;
    let connection =
        output
            .connect(&port, "orpheus-orca-out")
            .map_err(|error| TransportError::MidiConnect {
                port: port_name.to_owned(),
                reason: error.to_string(),
            })?;
    Ok(MidirSink { connection })
}

/// The reference velocity byte: `parseInt((velocity / 16) * 127)`, i.e.
/// `floor(velocity * 127 / 16)` with the grid's 0-16 velocity clamp.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // 16 * 127 / 16 = 127 <= u8::MAX
pub const fn velocity_byte(velocity: u8) -> u8 {
    let clamped = if velocity > 16 { 16 } else { velocity } as u16;
    (clamped * 127 / 16) as u8
}

/// Note-on bytes: `[0x90 + channel, note, velocity_byte]`.
#[must_use]
pub const fn note_on_bytes(channel: u8, note_id: u8, velocity: u8) -> [u8; 3] {
    [
        0x90 + (channel & 0x0F),
        note_id & 0x7F,
        velocity_byte(velocity),
    ]
}

/// Note-off bytes: `[0x80 + channel, note, velocity_byte]` (the reference
/// sends the item's velocity byte on release too).
#[must_use]
pub const fn note_off_bytes(channel: u8, note_id: u8, velocity: u8) -> [u8; 3] {
    [
        0x80 + (channel & 0x0F),
        note_id & 0x7F,
        velocity_byte(velocity),
    ]
}

/// Control-change bytes: `[0xB0 + channel, offset + knob, value]`
/// (`cc.js` sends `this.offset + msg.knob`; the sum is clamped to the MIDI
/// data-byte range).
#[must_use]
pub const fn cc_bytes(channel: u8, knob: u8, value: u8, offset: u8) -> [u8; 3] {
    let controller = offset.saturating_add(knob);
    [
        0xB0 + (channel & 0x0F),
        if controller > 0x7F { 0x7F } else { controller },
        value & 0x7F,
    ]
}

/// Pitch-bend bytes: `[0xE0 + channel, lsb, msb]` — the engine already
/// scaled both data bytes to 0-127.
#[must_use]
pub const fn pb_bytes(channel: u8, lsb: u8, msb: u8) -> [u8; 3] {
    [0xE0 + (channel & 0x0F), lsb & 0x7F, msb & 0x7F]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_byte_matches_reference_truncation() {
        // parseInt((v / 16) * 127) for v = 0, 8, 15 (default), 16 (ceiling).
        assert_eq!(velocity_byte(0), 0);
        assert_eq!(velocity_byte(8), 63);
        assert_eq!(velocity_byte(15), 119);
        assert_eq!(velocity_byte(16), 127);
        assert_eq!(velocity_byte(200), 127, "clamps above the grid ceiling");
    }

    #[test]
    fn note_bytes_carry_channel_and_velocity() {
        assert_eq!(note_on_bytes(1, 60, 15), [0x91, 60, 119]);
        assert_eq!(note_off_bytes(1, 60, 15), [0x81, 60, 119]);
    }

    #[test]
    fn cc_bytes_apply_the_knob_offset() {
        assert_eq!(cc_bytes(2, 3, 127, 64), [0xB2, 67, 127]);
        assert_eq!(cc_bytes(0, 0, 0, 64), [0xB0, 64, 0]);
    }

    #[test]
    fn pb_bytes_are_raw_data_bytes() {
        assert_eq!(pb_bytes(3, 11, 96), [0xE3, 11, 96]);
    }

    #[test]
    fn recording_sink_shares_its_buffer_across_clones() {
        let sink = RecordingMidiSink::new();
        let mut clone = sink.clone();
        clone.send(&[0x90, 60, 100]).expect("recording never fails");
        assert_eq!(sink.messages(), vec![vec![0x90, 60, 100]]);
    }
}
