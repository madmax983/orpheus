//! The `midi_export` module provides MIDI file generation for patterns.
//!
//! This module exports evaluated patterns into Standard MIDI Files (.mid).
//! It writes a basic Format 0 file without introducing external dependencies.

use std::io::Write;
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

/// Writes a Variable Length Quantity (VLQ) to a buffer.
/// Returns a slice of the written bytes.
#[allow(clippy::cast_possible_truncation)]
fn write_vlq(mut value: u32, buf: &mut [u8; 5]) -> usize {
    let mut i = 4;
    buf[i] = (value & 0x7F) as u8;
    while value > 0x7F {
        value >>= 7;
        i -= 1;
        buf[i] = ((value & 0x7F) | 0x80) as u8;
    }
    buf[i..].len()
}

/// Helper to map sample names to MIDI note numbers roughly.
fn sample_to_note(sample: &str) -> u8 {
    match sample {
        "bd" | "kick" => 36,       // Bass Drum 1
        "sn" | "snare" => 38,      // Acoustic Snare
        "hh" | "hat" | "ch" => 42, // Closed Hi Hat
        "oh" | "openhat" => 46,    // Open Hi Hat
        "cp" | "clap" => 39,       // Hand Clap
        "tom" => 45,               // Low Tom
        _ => 60,                   // Default to C4 if unknown
    }
}

/// Represents a simple MIDI event to be written to the track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MidiExportEvent {
    tick: u32,
    is_note_on: bool, // false = Note Off, true = Note On
    channel: u8,
    note: u8,
    velocity: u8,
}

impl PartialOrd for MidiExportEvent {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MidiExportEvent {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by tick first
        match self.tick.cmp(&other.tick) {
            std::cmp::Ordering::Equal => {
                // If on the same tick, sort Note Off (false) before Note On (true)
                self.is_note_on.cmp(&other.is_note_on)
            }
            other_cmp => other_cmp,
        }
    }
}

/// Exports a number pattern's evaluated events to a MIDI file (.mid).
/// Number patterns represent melodies, so they are routed to Channel 1.
pub fn export_number_pattern_to_midi(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;
    let path = path.as_ref();

    let tpqn = 960; // Ticks per quarter note
    let cycles_to_beats = 4.0; // Assume 4/4 time, 1 cycle = 4 quarter notes

    let mut midi_events = Vec::new();

    for event in &events {
        let start_f64 = f64::from(event.part.start());
        let end_f64 = f64::from(event.part.end());

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start_tick = (start_f64 * cycles_to_beats * f64::from(tpqn)).round() as u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let end_tick = (end_f64 * cycles_to_beats * f64::from(tpqn)).round() as u32;

        if start_tick >= end_tick {
            continue;
        }

        // Clamp note to valid MIDI range
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let note = event.value.clamp(0.0, 127.0).round() as u8;

        // Channel 1 is index 0
        midi_events.push(MidiExportEvent {
            tick: start_tick,
            is_note_on: true,
            channel: 0,
            note,
            velocity: 100, // Default velocity
        });

        midi_events.push(MidiExportEvent {
            tick: end_tick,
            is_note_on: false,
            channel: 0,
            note,
            velocity: 0,
        });
    }

    write_midi_file(path, midi_events, tpqn)
}

/// Exports a sample pattern's evaluated events to a MIDI file (.mid).
/// Sample patterns represent drums, so they are routed to Channel 10.
pub fn export_sample_pattern_to_midi(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let events = pattern.try_query(&span)?;
    let path = path.as_ref();

    let tpqn = 960; // Ticks per quarter note
    let cycles_to_beats = 4.0; // Assume 4/4 time, 1 cycle = 4 quarter notes

    let mut midi_events = Vec::new();

    for event in &events {
        let start_f64 = f64::from(event.part.start());
        let end_f64 = f64::from(event.part.end());

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start_tick = (start_f64 * cycles_to_beats * f64::from(tpqn)).round() as u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let end_tick = (end_f64 * cycles_to_beats * f64::from(tpqn)).round() as u32;

        if start_tick >= end_tick {
            continue;
        }

        let note = sample_to_note(event.value.sample());
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let velocity = (event.value.gain() * 127.0).clamp(0.0, 127.0).round() as u8;

        // Channel 10 is index 9 (Drum channel)
        midi_events.push(MidiExportEvent {
            tick: start_tick,
            is_note_on: true,
            channel: 9,
            note,
            velocity,
        });

        midi_events.push(MidiExportEvent {
            tick: end_tick,
            is_note_on: false,
            channel: 9,
            note,
            velocity: 0,
        });
    }

    write_midi_file(path, midi_events, tpqn)
}

fn write_midi_file(
    path: &Path,
    mut midi_events: Vec<MidiExportEvent>,
    tpqn: u16,
) -> Result<(), EvalError> {
    // Sort events correctly (ticks, then Note Offs before Note Ons)
    midi_events.sort();

    let mut file = std::fs::File::create(path)?;

    // MThd chunk
    file.write_all(b"MThd")?;
    file.write_all(&6u32.to_be_bytes())?; // Chunk length
    file.write_all(&0u16.to_be_bytes())?; // Format 0
    file.write_all(&1u16.to_be_bytes())?; // 1 track
    file.write_all(&tpqn.to_be_bytes())?; // Ticks per quarter note

    // MTrk chunk
    let mut track_data = Vec::new();
    let mut current_tick = 0;
    let mut vlq_buf = [0u8; 5];

    // Tempo meta event: 120 BPM (500000 microseconds per quarter note)
    let delta_bytes = {
        let len = write_vlq(0, &mut vlq_buf);
        vlq_buf[5 - len..].to_vec()
    };
    track_data.extend_from_slice(&delta_bytes);
    track_data.push(0xFF); // Meta event
    track_data.push(0x51); // Tempo
    track_data.push(0x03); // Length
    track_data.extend_from_slice(&500_000u32.to_be_bytes()[1..4]);

    // Write note events
    for event in midi_events {
        let delta = event.tick.saturating_sub(current_tick);
        current_tick = event.tick;

        let delta_bytes = {
            let len = write_vlq(delta, &mut vlq_buf);
            vlq_buf[5 - len..].to_vec()
        };
        track_data.extend_from_slice(&delta_bytes);

        let status = if event.is_note_on {
            0x90 | (event.channel & 0x0F)
        } else {
            0x80 | (event.channel & 0x0F)
        };

        track_data.push(status);
        track_data.push(event.note & 0x7F);
        track_data.push(event.velocity & 0x7F);
    }

    // End of track meta event
    let delta_bytes = {
        let len = write_vlq(0, &mut vlq_buf);
        vlq_buf[5 - len..].to_vec()
    };
    track_data.extend_from_slice(&delta_bytes);
    track_data.push(0xFF);
    track_data.push(0x2F);
    track_data.push(0x00);

    // Write MTrk header and data
    file.write_all(b"MTrk")?;
    #[allow(clippy::cast_possible_truncation)]
    let len_u32 = track_data.len() as u32;
    file.write_all(&len_u32.to_be_bytes())?;
    file.write_all(&track_data)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static UNIQUE_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    fn temp_mid_path() -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let counter = UNIQUE_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("orpheus-export-{timestamp}-{counter}.mid"))
    }

    #[test]
    fn write_vlq_encodes_correctly() {
        let mut buf = [0u8; 5];
        let len = write_vlq(0, &mut buf);
        assert_eq!(&buf[5 - len..], &[0x00]);

        let mut buf = [0u8; 5];
        let len = write_vlq(127, &mut buf);
        assert_eq!(&buf[5 - len..], &[0x7F]);

        let mut buf = [0u8; 5];
        let len = write_vlq(128, &mut buf);
        assert_eq!(&buf[5 - len..], &[0x81, 0x00]);
    }

    #[test]
    fn midi_exporter_generates_sample_pattern() {
        let source = "pattern = fast(2, bd sn)";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_sample_pattern().unwrap();

        let path = temp_mid_path();
        export_sample_pattern_to_midi(pattern, &path, 1).unwrap();

        let content = fs::read(&path).unwrap();
        assert_eq!(&content[0..4], b"MThd");
        assert_eq!(&content[14..18], b"MTrk");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn midi_exporter_generates_number_pattern() {
        let source = "pattern = 60 62 64";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("pattern").unwrap().as_number_pattern().unwrap();

        let path = temp_mid_path();
        export_number_pattern_to_midi(pattern, &path, 1).unwrap();

        let content = fs::read(&path).unwrap();
        assert_eq!(&content[0..4], b"MThd");

        let _ = fs::remove_file(path);
    }
}
