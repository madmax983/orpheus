use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use crate::eval::{EvalError, render_span};
use crate::value::{NumberPatternValue, SamplePatternValue};

const TICKS_PER_QUARTER_NOTE: u16 = 480;
// One cycle = 4 quarter notes (assuming 4/4 meter)
const TICKS_PER_CYCLE: u32 = (TICKS_PER_QUARTER_NOTE as u32) * 4;

/// Write a variable length quantity (VLQ) as used in standard MIDI files.
fn write_vlq(mut val: u32, w: &mut impl Write) -> std::io::Result<()> {
    let mut buf = [0u8; 5];
    let mut i = 4;
    buf[i] = (val & 0x7F) as u8;
    while val > 0x7F {
        val >>= 7;
        i = i.saturating_sub(1);
        buf[i] = ((val & 0x7F) | 0x80) as u8;
    }
    w.write_all(&buf[i..])
}

fn write_midi_file(
    path: impl AsRef<Path>,
    events: &[(u32, bool, u8, u8)], // (tick, is_note_on, note, velocity)
    channel: u8,
) -> Result<(), EvalError> {
    let mut file = BufWriter::new(
        File::create(path).map_err(|e| EvalError::new(&*format!("failed to create file: {e}")))?,
    );

    // MThd chunk
    file.write_all(b"MThd")
        .map_err(|e| EvalError::new(&*e.to_string()))?;
    file.write_all(&[0, 0, 0, 6])
        .map_err(|e| EvalError::new(&*e.to_string()))?; // length 6
    file.write_all(&[0, 0])
        .map_err(|e| EvalError::new(&*e.to_string()))?; // format 0
    file.write_all(&[0, 1])
        .map_err(|e| EvalError::new(&*e.to_string()))?; // 1 track
    file.write_all(&TICKS_PER_QUARTER_NOTE.to_be_bytes())
        .map_err(|e| EvalError::new(&*e.to_string()))?;

    // MTrk chunk
    let mut track_data = Vec::new();

    // Optional: Set tempo (120 BPM = 500,000 microseconds per quarter note)
    write_vlq(0, &mut track_data).map_err(|e| EvalError::new(&*e.to_string()))?;
    track_data.extend_from_slice(&[0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20]);

    let mut last_tick = 0;
    for &(tick, is_on, note, velocity) in events {
        let delta = tick.saturating_sub(last_tick);
        write_vlq(delta, &mut track_data).map_err(|e| EvalError::new(&*e.to_string()))?;
        let ch = channel.clamp(0, 15);
        if is_on {
            track_data.push(0x90 | ch);
        } else {
            track_data.push(0x80 | ch);
        }
        track_data.push(note.clamp(0, 127));
        track_data.push(velocity.clamp(0, 127));
        last_tick = tick;
    }

    // End of track
    write_vlq(0, &mut track_data).map_err(|e| EvalError::new(&*e.to_string()))?;
    track_data.extend_from_slice(&[0xFF, 0x2F, 0x00]);

    file.write_all(b"MTrk")
        .map_err(|e| EvalError::new(&*e.to_string()))?;
    file.write_all(&(track_data.len() as u32).to_be_bytes())
        .map_err(|e| EvalError::new(&*e.to_string()))?;
    file.write_all(&track_data)
        .map_err(|e| EvalError::new(&*e.to_string()))?;

    Ok(())
}

/// Exports a number pattern to a Standard MIDI File format 0.
///
/// Converts the pattern into MIDI notes. The values are interpreted as MIDI note
/// numbers (0-127). The track will use channel 1.
///
/// # Errors
/// Returns [`EvalError`] if the export fails or if `cycle_count` is 0.
pub fn export_number_pattern_to_midi(
    pattern: &NumberPatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let pattern_events = pattern.try_query(&span)?;

    let mut midi_events = Vec::new();

    for event in pattern_events {
        let note = event.value.round().clamp(0.0, 127.0) as u8;
        let start_cycle = f64::from(event.part.start());
        let end_cycle = f64::from(event.part.end());

        if end_cycle > start_cycle {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let start_tick = (start_cycle * f64::from(TICKS_PER_CYCLE)).round() as u32;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let end_tick = (end_cycle * f64::from(TICKS_PER_CYCLE)).round() as u32;

            midi_events.push((start_tick, true, note, 100)); // Velocity 100
            midi_events.push((end_tick, false, note, 0));
        }
    }

    midi_events.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1)) // Note Offs before Note Ons (false < true)
            .then_with(|| left.2.cmp(&right.2))
    });

    write_midi_file(path, &midi_events, 0)
}

/// Exports a sample pattern to a Standard MIDI File format 0.
///
/// Maps generic drum sample names (e.g., `bd`, `sn`, `hh`) to General MIDI
/// percussion note numbers on Channel 10.
///
/// # Errors
/// Returns [`EvalError`] if the export fails or if `cycle_count` is 0.
pub fn export_sample_pattern_to_midi(
    pattern: &SamplePatternValue,
    path: impl AsRef<Path>,
    cycle_count: u64,
) -> Result<(), EvalError> {
    if cycle_count == 0 {
        return Err(EvalError::new("exporting requires at least one cycle"));
    }

    let span = render_span(cycle_count)?;
    let pattern_events = pattern.try_query(&span)?;

    let mut midi_events = Vec::new();

    for event in pattern_events {
        let sample = event.value.sample();
        let note = match sample {
            "bd" | "kick" => 36,
            "sn" | "snare" => 38,
            "hh" | "hat" => 42,
            "oh" | "openhat" => 46,
            "cp" | "clap" => 39,
            "cr" | "crash" => 49,
            "rd" | "ride" => 51,
            "tom" | "lt" => 43, // Low Tom
            "mt" => 47,         // Mid Tom
            "ht" => 50,         // High Tom
            _ => 60,            // Default fallback note (C4)
        };

        let start_cycle = f64::from(event.part.start());
        let end_cycle = f64::from(event.part.end());

        if end_cycle > start_cycle {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let start_tick = (start_cycle * f64::from(TICKS_PER_CYCLE)).round() as u32;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let end_tick = (end_cycle * f64::from(TICKS_PER_CYCLE)).round() as u32;

            // Map gain to velocity
            let gain = event.value.gain();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let velocity = (gain * 100.0).round().clamp(0.0, 127.0) as u8;

            midi_events.push((start_tick, true, note, velocity));
            // Note off
            midi_events.push((end_tick, false, note, 0));
        }
    }

    midi_events.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1)) // Note Offs before Note Ons (false < true)
            .then_with(|| left.2.cmp(&right.2))
    });

    write_midi_file(path, &midi_events, 9)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};
    use std::fs;

    fn temp_midi_path() -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("orpheus-test-{unique}.mid"))
    }

    #[test]
    fn midi_export_generates_valid_file_for_numbers() {
        let module = eval_module("pat = 60 62 64 65", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_number_pattern().unwrap();
        let path = temp_midi_path();

        export_number_pattern_to_midi(pat, &path, 1).unwrap();

        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"MThd"));
        assert!(bytes.windows(4).any(|w| w == b"MTrk"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn midi_export_generates_valid_file_for_samples() {
        let module = eval_module("pat = bd sn hh cp", ReplMode::Loose).unwrap();
        let pat = module.get("pat").unwrap().as_sample_pattern().unwrap();
        let path = temp_midi_path();

        export_sample_pattern_to_midi(pat, &path, 1).unwrap();

        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"MThd"));
        assert!(bytes.windows(4).any(|w| w == b"MTrk"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn vlq_writing_handles_multi_byte_values() {
        let mut buf = Vec::new();
        write_vlq(0, &mut buf).unwrap();
        assert_eq!(buf, vec![0x00]);

        buf.clear();
        write_vlq(127, &mut buf).unwrap();
        assert_eq!(buf, vec![0x7F]);

        buf.clear();
        write_vlq(128, &mut buf).unwrap();
        assert_eq!(buf, vec![0x81, 0x00]);

        buf.clear();
        write_vlq(106903, &mut buf).unwrap();
        assert_eq!(buf, vec![0x86, 0xC3, 0x17]);
    }
}
