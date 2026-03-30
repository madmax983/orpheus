//! The `midi` module provides MIDI export format for evaluated number patterns.
//!
//! This exporter generates a standard MIDI file (SMF) from a pitched number pattern,
//! allowing melodies and chords to be imported into DAWs or external synthesizers.

use std::path::Path;

use midly::{
    Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
};

use crate::eval::{EvalError, render_span};
use crate::value::NumberPatternValue;

/// Exports a number pattern's evaluated events to a MIDI file (`.mid`).
///
/// Each numerical event is treated as a MIDI note number (0-127). Events outside
/// this range are clamped. Time is quantized to a fixed resolution of 480 ticks
/// per quarter note (where one cycle is treated as a whole note, i.e., 4 quarter notes).
///
/// # Errors
///
/// Returns [`EvalError`] if pattern querying fails or if the file cannot be written.
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

    // Standard resolution: 480 ticks per quarter note
    // Assuming 1 cycle = 1 measure (4/4 time) = 4 quarter notes = 1920 ticks
    let ticks_per_cycle = 1920_f64;

    let mut midi_events = Vec::new();

    // Collect all note on and off events with their absolute times in ticks
    for event in events {
        let start_cycle = f64::from(event.part.start());
        let end_cycle = f64::from(event.part.end());

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start_tick = (start_cycle * ticks_per_cycle).round() as u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let end_tick = (end_cycle * ticks_per_cycle).round() as u32;

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let note_number = if event.value < 0.0 {
            0
        } else if event.value > 127.0 {
            127
        } else {
            event.value.round() as u8
        };

        midi_events.push((start_tick, true, note_number));
        midi_events.push((end_tick, false, note_number));
    }

    // Sort events by absolute tick time, note off before note on if at same time
    midi_events.sort_by(|a, b| {
        a.0.cmp(&b.0).then_with(|| match (a.1, b.1) {
            (true, true) | (false, false) => std::cmp::Ordering::Equal,
            (false, true) => std::cmp::Ordering::Less,    // NoteOff before NoteOn
            (true, false) => std::cmp::Ordering::Greater, // NoteOn after NoteOff
        })
    });

    let mut track = Vec::new();
    let mut current_tick = 0;

    // Set tempo (e.g., 120 BPM = 500,000 microseconds per quarter note)
    track.push(TrackEvent {
        delta: 0.into(),
        kind: TrackEventKind::Meta(MetaMessage::Tempo(500_000.into())),
    });

    for (tick, is_on, note) in midi_events {
        let delta = tick.saturating_sub(current_tick);
        current_tick = tick;

        let message = if is_on {
            MidiMessage::NoteOn {
                key: note.into(),
                vel: 100.into(), // Default velocity
            }
        } else {
            MidiMessage::NoteOff {
                key: note.into(),
                vel: 0.into(),
            }
        };

        track.push(TrackEvent {
            delta: delta.into(),
            kind: TrackEventKind::Midi {
                channel: 0.into(),
                message,
            },
        });
    }

    // End of track meta message
    track.push(TrackEvent {
        delta: 0.into(),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });

    let header = Header {
        format: Format::SingleTrack,
        timing: Timing::Metrical(480.into()),
    };

    let smf = Smf {
        header,
        tracks: vec![track],
    };

    smf.save(path)
        .map_err(|e| EvalError::new(format!("failed to write MIDI file: {e}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ReplMode, eval_module};

    #[test]
    fn midi_exporter_generates_valid_midi_file() {
        let source = "melody = c4 e4 g4 c5";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("melody").unwrap().as_number_pattern().unwrap();

        let path = std::env::temp_dir().join("test_output.mid");
        export_number_pattern_to_midi(pattern, &path, 1).unwrap();

        let data = std::fs::read(&path).unwrap();
        let smf = Smf::parse(&data).unwrap();

        assert_eq!(smf.header.format, Format::SingleTrack);
        assert_eq!(smf.tracks.len(), 1);

        let track = &smf.tracks[0];
        // Ensure there are some NoteOn and NoteOff events
        let note_ons = track.iter().filter(|ev| matches!(ev.kind, TrackEventKind::Midi { message: MidiMessage::NoteOn { .. }, .. })).count();
        let note_offs = track.iter().filter(|ev| matches!(ev.kind, TrackEventKind::Midi { message: MidiMessage::NoteOff { .. }, .. })).count();

        assert_eq!(note_ons, 4);
        assert_eq!(note_offs, 4);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn export_cycle_count_zero_returns_error() {
        let source = "melody = c4 e4 g4";
        let module = eval_module(source, ReplMode::Loose).unwrap();
        let pattern = module.get("melody").unwrap().as_number_pattern().unwrap();

        let result = export_number_pattern_to_midi(pattern, "test_output.mid", 0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "exporting requires at least one cycle"
        );
    }
}
