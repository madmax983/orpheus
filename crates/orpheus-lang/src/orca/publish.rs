//! Publish bridge: [`OrcaEvent`]s into session-publishable event batches.
//!
//! This is the v1 seam described in `docs/design/orca-surface.md` section 1.2
//! and ADR 0008: a materialized grid cycle is a unit-cycle
//! `Vec<Event<SampleEvent>>`, exactly the shape the existing
//! `SamplePatternValue::from_events` -> `ReplSession::publish_sample_events`
//! path consumes. Because a running grid is generally *not* cycle-periodic,
//! [`OrcaPublisher::poll`] re-materializes and re-publishes the next cycle's
//! events at every engine cycle boundary instead of publishing once.
//!
//! Note mapping (v3, superseding the base-36 mapping of ADR 0008): a MIDI
//! note event's glyph and octave are transposed to a MIDI note number via
//! [`midi_note_id`] (the exact reference `transpose.js`/`io/midi.js` table,
//! lowercase glyphs are sharps) and interpreted as a semitone offset
//! relative to middle C (60), applied as the playback-rate multiplier
//! `2^(semitones / 12)`. `:03C` therefore plays the sample token at its base
//! pitch. Non-note IO events (CC, pitch bend, UDP, OSC, `$` commands) stay
//! on the engine's per-tick event list for future transports and do not
//! reach the audio path.
//!
//! v4 consumes the note's velocity and length ports (see
//! `docs/design/orca-surface.md` section 10): velocity becomes the event's
//! linear gain through the reference `io/midi.js` scaling
//! `floor(velocity * 127 / 16) / 127`, and length `L` stretches the event's
//! span to `L` grid frames, clamped at the cycle end because the per-cycle
//! re-publish model (ADR 0008) cannot sustain a note across a boundary.

use orpheus_pattern::{Event, PatternError, Rational, TimeSpan};

use crate::value::SampleEvent;

use super::engine::{OrcaEngine, OrcaEvent, OrcaIoEvent};
use super::grid::Grid;

/// Default grid frames per musical cycle: Orca's convention of 16th-note
/// frames over a 4-beat bar.
pub const DEFAULT_GRID_FRAMES_PER_CYCLE: u64 = 16;

/// Default sample token triggered by grid note events (a pitched synth
/// primitive from the sample-identifier whitelist).
pub const DEFAULT_SAMPLE_TOKEN: &str = "tri";

/// The session binding name under which grid cycles are published.
pub const ORCA_PATTERN_NAME: &str = "orca";

/// Default grid width for a freshly spawned surface (one column per frame at
/// the default 16 frames per cycle).
pub const DEFAULT_GRID_WIDTH: usize = 16;

/// Default grid height for a freshly spawned surface.
pub const DEFAULT_GRID_HEIGHT: usize = 8;

/// The MIDI note number the audio bridge treats as the sample token's base
/// pitch: middle C, i.e. `:03C`.
const MIDDLE_C: i16 = 60;

/// Chromatic index (0-11, where odd indices between naturals are sharps)
/// and octave offset for a note glyph, transcribing the reference
/// `transpose.js` table exactly. Uppercase letters are naturals, lowercase
/// are sharps; letters past `G` wrap upward through the octaves, and the
/// nonexistent sharps `e`/`l`/`s`/`z` and `b`/`i`/`p`/`w` "catch" to the
/// next natural (`F` and `C` respectively).
const fn transpose_entry(note: char) -> Option<(u8, u8)> {
    // Chromatic order within an octave: C c D d E F f G g A a B.
    Some(match note {
        'C' => (0, 0),
        'c' => (1, 0),
        'D' => (2, 0),
        'd' => (3, 0),
        'E' => (4, 0),
        'F' | 'e' => (5, 0),
        'f' => (6, 0),
        'G' => (7, 0),
        'g' => (8, 0),
        'A' | 'H' => (9, 0),
        'a' | 'h' => (10, 0),
        'B' | 'I' => (11, 0),
        'J' | 'b' | 'i' => (0, 1),
        'j' => (1, 1),
        'K' => (2, 1),
        'k' => (3, 1),
        'L' => (4, 1),
        'M' | 'l' => (5, 1),
        'm' => (6, 1),
        'N' => (7, 1),
        'n' => (8, 1),
        'O' => (9, 1),
        'o' => (10, 1),
        'P' => (11, 1),
        'Q' | 'p' => (0, 2),
        'q' => (1, 2),
        'R' => (2, 2),
        'r' => (3, 2),
        'S' => (4, 2),
        'T' | 's' => (5, 2),
        't' => (6, 2),
        'U' => (7, 2),
        'u' => (8, 2),
        'V' => (9, 2),
        'v' => (10, 2),
        'W' => (11, 2),
        'X' | 'w' => (0, 3),
        'x' => (1, 3),
        'Y' => (2, 3),
        'y' => (3, 3),
        'Z' => (4, 3),
        'z' => (5, 3),
        _ => return None,
    })
}

/// The MIDI note number for a note glyph at `octave`.
///
/// Matches the reference `io/midi.js` `transpose()`: the glyph's octave
/// offset is added and clamped to 0-8, then the result is
/// `octave * 12 + chromatic + 24`, clamped to 127. Returns `None` for
/// glyphs outside the transpose table (the reference silently drops those
/// notes at send time).
#[must_use]
pub const fn midi_note_id(note: char, octave: u8) -> Option<u8> {
    let Some((chromatic, offset)) = transpose_entry(note) else {
        return None;
    };
    let octave = min_u8(octave.saturating_add(offset), 8);
    // Max is 8 * 12 + 11 + 24 = 131: no overflow in u8 arithmetic.
    Some(min_u8(octave * 12 + chromatic + 24, 127))
}

/// `const`-compatible `u8::min`.
const fn min_u8(value: u8, ceiling: u8) -> u8 {
    if value < ceiling { value } else { ceiling }
}

/// The linear gain for a grid velocity (the `:`/`%` velocity port, 0-16).
///
/// The reference `io/midi.js` sends `parseInt((velocity / 16) * 127)` as the
/// MIDI velocity byte; that byte maps linearly onto Orpheus's gain (an
/// amplitude multiplier where `1.0` is full volume), so the default velocity
/// `f` (15) plays at `119/127` and the ceiling `g` (16) at exactly `1.0`.
/// The linear curve (rather than the common `(v/127)^2`) matches the `gain`
/// transform's convention that `0.5` means half amplitude.
fn velocity_gain(velocity: u8) -> f64 {
    let byte = u32::from(min_u8(velocity, 16)) * 127 / 16;
    f64::from(byte) / 127.0
}

/// The `(part, whole)` spans for a note fired at `frame_in_cycle` lasting
/// `length` grid frames of `frames_per_cycle`.
///
/// The reference `io/midi.js` presses a note on its frame and releases it
/// once `length` frames have elapsed, so a note with length `L` sounds for
/// exactly `L` frames: `[N/F, (N+L)/F)`. A length of `0` (press and release
/// within the same frame pass) collapses to one frame, the shortest span the
/// unit-cycle event model can carry. When the span crosses the cycle end the
/// playable `part` is clamped at `1` and the full extent is preserved as the
/// event's `whole` — the per-cycle re-publish model re-materializes the next
/// cycle from scratch, so the truncated tail never sounds (a documented
/// limitation, not reference behavior).
fn note_spans(
    frame_in_cycle: u64,
    length: u8,
    frames_per_cycle: u64,
) -> Result<(TimeSpan, Option<TimeSpan>), PatternError> {
    let length_frames = i128::from(length.max(1));
    let denominator = i128::from(frames_per_cycle);
    let start = Rational::checked_from_parts(i128::from(frame_in_cycle), denominator)?;
    let end =
        Rational::checked_from_parts(i128::from(frame_in_cycle) + length_frames, denominator)?;
    let cycle_end = Rational::one();
    if start < cycle_end && end > cycle_end {
        let part = TimeSpan::new(start, cycle_end)?;
        let whole = TimeSpan::new(start, end)?;
        Ok((part, Some(whole)))
    } else {
        Ok((TimeSpan::new(start, end)?, None))
    }
}

/// Converts one [`OrcaEvent`] fired at `frame_in_cycle` of `frames_per_cycle`
/// into an unclipped unit-cycle [`Event<SampleEvent>`], or `None` when the
/// event does not reach the audio path.
///
/// Only the MIDI-note family (`:` [`OrcaIoEvent::Midi`], `%`
/// [`OrcaIoEvent::MidiMono`]) becomes audible; other IO events — and notes
/// whose glyph falls outside the transpose table — return `Ok(None)`. A
/// note with length `L` occupies the exact rational span
/// `[frame_in_cycle / frames_per_cycle, (frame_in_cycle + L) / frames_per_cycle)`
/// (`L = 0` collapses to one frame; a span crossing the cycle end keeps its
/// full extent as `whole` while `part` is clamped at `1`) and triggers
/// `sample_token` repitched by the note's [`midi_note_id`] relative to
/// middle C (`:03C` plays the base pitch), at the linear gain
/// `floor(velocity * 127 / 16) / 127` — the reference `io/midi.js` velocity
/// byte mapped onto Orpheus's linear amplitude convention.
///
/// # Errors
///
/// Returns [`PatternError::InvalidDenominator`] when `frames_per_cycle` is
/// zero, and propagates arithmetic errors from rational construction.
pub fn sample_event_from_orca(
    event: &OrcaEvent,
    frame_in_cycle: u64,
    frames_per_cycle: u64,
    sample_token: &str,
) -> Result<Option<Event<SampleEvent>>, PatternError> {
    let (OrcaIoEvent::Midi(note) | OrcaIoEvent::MidiMono(note)) = &event.io else {
        return Ok(None);
    };
    let Some(id) = midi_note_id(note.note, note.octave) else {
        return Ok(None);
    };
    let (part, whole) = note_spans(frame_in_cycle, note.length, frames_per_cycle)?;
    let semitones = f64::from(i16::from(id) - MIDDLE_C);
    let value = SampleEvent::named(sample_token)
        .repitched(semitones)
        .with_gain(velocity_gain(note.velocity));
    Ok(Some(Event { whole, part, value }))
}

/// Advances `engine` by one full grid cycle (`frames_per_cycle` ticks) and
/// returns the emitted MIDI-note events stamped with their cycle-relative
/// frame spans.
///
/// Events are ordered by frame, and within a frame by the engine's row-major
/// scan order; same-frame events share a span. Non-note IO events (CC,
/// pitch bend, UDP, OSC, commands) are skipped — they remain on the engine's
/// per-tick event list for future transport consumers. Calling this
/// repeatedly yields consecutive grid cycles: the grid state carries over,
/// which is what makes non-periodic grids (a moving `E`) evolve across
/// cycles.
///
/// # Errors
///
/// Propagates rational-construction errors from note-span construction. A
/// `frames_per_cycle` of zero ticks nothing and returns an empty batch.
pub fn materialize_cycle(
    engine: &mut OrcaEngine,
    frames_per_cycle: u64,
    sample_token: &str,
) -> Result<Vec<Event<SampleEvent>>, PatternError> {
    let mut events = Vec::new();
    for frame_in_cycle in 0..frames_per_cycle {
        for orca_event in engine.tick() {
            if let Some(event) =
                sample_event_from_orca(orca_event, frame_in_cycle, frames_per_cycle, sample_token)?
            {
                events.push(event);
            }
        }
    }
    Ok(events)
}

/// Maps the audio engine's transport position onto the current grid frame.
///
/// `current_frame` and `cycle_start_frame` are absolute audio frames from
/// `TransportSnapshot`; `engine_frames_per_cycle` is the audio frames per
/// musical cycle and `grid_frames_per_cycle` the grid's frame count `F`. The
/// result is clamped to `0..grid_frames_per_cycle` and is `None` when either
/// clock is degenerate (zero frames per cycle).
#[must_use]
pub const fn playhead_frame(
    current_frame: u64,
    cycle_start_frame: u64,
    engine_frames_per_cycle: u64,
    grid_frames_per_cycle: u64,
) -> Option<u64> {
    if engine_frames_per_cycle == 0 || grid_frames_per_cycle == 0 {
        return None;
    }
    let offset = current_frame.saturating_sub(cycle_start_frame);
    let frame = offset.saturating_mul(grid_frames_per_cycle) / engine_frames_per_cycle;
    if frame >= grid_frames_per_cycle {
        Some(grid_frames_per_cycle - 1)
    } else {
        Some(frame)
    }
}

/// Drives grid materialization from engine cycle boundaries.
///
/// The publisher owns the grid engine and a boundary tracker. The hosting
/// surface polls it with the latest `TransportSnapshot::current_cycle_start_frame`;
/// on the first poll after [`Self::start`] and on every boundary change it
/// materializes the next grid cycle for publication one cycle ahead of
/// playback (the engine adopts published patterns at the following boundary).
#[derive(Clone, Debug)]
pub struct OrcaPublisher {
    engine: OrcaEngine,
    frames_per_cycle: u64,
    sample_token: String,
    running: bool,
    last_cycle_start: Option<u64>,
}

impl OrcaPublisher {
    /// Wraps a grid engine with a cycle clock of `frames_per_cycle` grid
    /// frames per musical cycle, triggering `sample_token` for note events.
    #[must_use]
    pub fn new(engine: OrcaEngine, frames_per_cycle: u64, sample_token: &str) -> Self {
        Self {
            engine,
            frames_per_cycle,
            sample_token: sample_token.to_owned(),
            running: false,
            last_cycle_start: None,
        }
    }

    /// A stopped publisher over an empty default-sized grid.
    ///
    /// # Panics
    ///
    /// Never panics in practice: the default grid dimensions are nonzero.
    #[must_use]
    pub fn with_default_grid() -> Self {
        let grid = Grid::new(DEFAULT_GRID_WIDTH, DEFAULT_GRID_HEIGHT)
            .expect("default grid dimensions are nonzero");
        Self::new(
            OrcaEngine::new(grid),
            DEFAULT_GRID_FRAMES_PER_CYCLE,
            DEFAULT_SAMPLE_TOKEN,
        )
    }

    /// The wrapped grid engine.
    #[must_use]
    pub const fn engine(&self) -> &OrcaEngine {
        &self.engine
    }

    /// Mutable access to the grid engine, for editing between cycles.
    pub const fn engine_mut(&mut self) -> &mut OrcaEngine {
        &mut self.engine
    }

    /// Grid frames per musical cycle.
    #[must_use]
    pub const fn frames_per_cycle(&self) -> u64 {
        self.frames_per_cycle
    }

    /// Whether the grid clock is running (publishing at cycle boundaries).
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Starts the grid clock. The next [`Self::poll`] publishes immediately,
    /// regardless of the last seen boundary.
    pub const fn start(&mut self) {
        self.running = true;
        self.last_cycle_start = None;
    }

    /// Stops the grid clock; subsequent polls publish nothing.
    pub const fn stop(&mut self) {
        self.running = false;
    }

    /// Reports the current engine cycle boundary and returns the next grid
    /// cycle's events when they must be (re-)published.
    ///
    /// Returns `Ok(None)` while stopped or while the boundary is unchanged
    /// since the previous poll; returns `Ok(Some(events))` on the first poll
    /// after [`Self::start`] and whenever `cycle_start_frame` differs from the
    /// previous poll.
    ///
    /// # Errors
    ///
    /// Propagates rational-construction errors from materialization.
    pub fn poll(
        &mut self,
        cycle_start_frame: u64,
    ) -> Result<Option<Vec<Event<SampleEvent>>>, PatternError> {
        if !self.running || self.last_cycle_start == Some(cycle_start_frame) {
            return Ok(None);
        }
        self.last_cycle_start = Some(cycle_start_frame);
        materialize_cycle(&mut self.engine, self.frames_per_cycle, &self.sample_token).map(Some)
    }
}
