//! Behavior tests for Orca v4: velocity-driven gain, note length as event
//! duration, and the `#` comment operator.
//!
//! Semantics are pinned against main-branch `hundredrabbits/Orca`:
//! `io/midi.js` sends velocity as `parseInt((velocity / 16) * 127)` and
//! counts length down one frame per `run()` (note-on at frame `N`, note-off
//! at frame `N + L`); `library.js` `OperatorComment` locks its own row
//! eastward up to and including the matching `#` (or the row end when
//! unmatched). See `docs/design/orca-surface.md` section 10.

use orpheus_lang::orca::{
    MidiNote, OrcaEngine, OrcaEvent, OrcaIoEvent, is_valid_glyph, materialize_cycle,
    sample_event_from_orca,
};
use orpheus_pattern::Rational;

fn engine(rows: &[&str]) -> OrcaEngine {
    OrcaEngine::from_rows(rows).expect("test grids are well-formed")
}

fn rows(engine: &OrcaEngine) -> Vec<String> {
    engine.grid().rows()
}

const fn note_event(velocity: u8, length: u8) -> OrcaEvent {
    OrcaEvent {
        frame: 0,
        x: 0,
        y: 0,
        io: OrcaIoEvent::Midi(MidiNote {
            channel: 0,
            octave: 3,
            note: 'C',
            velocity,
            length,
        }),
    }
}

fn rational(numerator: i64, denominator: i64) -> Rational {
    Rational::new(numerator, denominator).expect("valid rational")
}

// ---------------------------------------------------------------------------
// Velocity -> gain (reference: midi.js `parseInt((velocity / 16) * 127)`,
// then linear amplitude, matching Orpheus's linear `gain` convention).
// ---------------------------------------------------------------------------

#[test]
fn default_velocity_maps_to_reference_gain() {
    // Empty velocity port defaults to `f` (15): MIDI velocity
    // floor(15 * 127 / 16) = 119, so gain = 119/127.
    let event = sample_event_from_orca(&note_event(15, 1), 0, 16, "tri")
        .expect("valid span")
        .expect("note events map to sample events");
    let expected = 119.0 / 127.0;
    assert!((event.value.gain() - expected).abs() < 1e-12);
}

#[test]
fn max_velocity_is_unity_gain() {
    // Velocity `g` (16) is the clamp ceiling: floor(16 * 127 / 16) = 127.
    let event = sample_event_from_orca(&note_event(16, 1), 0, 16, "tri")
        .expect("valid span")
        .expect("note event");
    assert!((event.value.gain() - 1.0).abs() < 1e-12);
}

#[test]
fn zero_velocity_is_silent() {
    let event = sample_event_from_orca(&note_event(0, 1), 0, 16, "tri")
        .expect("valid span")
        .expect("note event");
    assert!((event.value.gain() - 0.0).abs() < 1e-12);
}

#[test]
fn velocity_scales_gain_linearly_with_truncation() {
    // Velocity 8: parseInt((8 / 16) * 127) = parseInt(63.5) = 63.
    let event = sample_event_from_orca(&note_event(8, 1), 0, 16, "tri")
        .expect("valid span")
        .expect("note event");
    let expected = 63.0 / 127.0;
    assert!((event.value.gain() - expected).abs() < 1e-12);
}

#[test]
fn materialized_cycle_carries_velocity_gain() {
    // Explicit velocity `8` in the `:` velocity port (offset {4,0}).
    let mut orca = engine(&[".D1....", "..:04c8"]);
    let events = materialize_cycle(&mut orca, 1, "tri").expect("materializes");
    assert_eq!(events.len(), 1);
    assert!((events[0].value.gain() - 63.0 / 127.0).abs() < 1e-12);
}

// ---------------------------------------------------------------------------
// Length -> event duration (reference: midi.js `run()` presses on the note's
// frame and releases once length frames have elapsed, so a note with length
// L spans L grid frames).
// ---------------------------------------------------------------------------

#[test]
fn default_length_spans_one_frame() {
    let event = sample_event_from_orca(&note_event(15, 1), 3, 16, "tri")
        .expect("valid span")
        .expect("note event");
    assert_eq!(event.whole, None, "unclipped events carry no whole");
    assert_eq!(event.part.start(), &rational(3, 16));
    assert_eq!(event.part.end(), &rational(4, 16));
}

#[test]
fn length_extends_event_span_by_frames() {
    let event = sample_event_from_orca(&note_event(15, 4), 2, 16, "tri")
        .expect("valid span")
        .expect("note event");
    assert_eq!(event.whole, None);
    assert_eq!(event.part.start(), &rational(2, 16));
    assert_eq!(event.part.end(), &rational(6, 16));
}

#[test]
fn length_crossing_cycle_end_clips_part_and_keeps_whole() {
    // Frame 14 of 16 with length 8 sustains to 22/16: the part is clipped
    // at the published cycle window's end (Tidal-style) while the whole
    // records the full extent. As of v5 (ADR 0009) the engine scheduler
    // derives the trigger duration from the whole, so the note is audible
    // across the boundary — see tests/orca_v5.rs.
    let event = sample_event_from_orca(&note_event(15, 8), 14, 16, "tri")
        .expect("valid span")
        .expect("note event");
    assert_eq!(event.part.start(), &rational(14, 16));
    assert_eq!(event.part.end(), &rational(1, 1));
    let whole = event.whole.expect("clipped events keep their whole");
    assert_eq!(whole.start(), &rational(14, 16));
    assert_eq!(whole.end(), &rational(22, 16));
}

#[test]
fn zero_length_collapses_to_one_frame() {
    // The reference presses and releases within the same frame pass; the
    // shortest representable event here is one grid frame.
    let event = sample_event_from_orca(&note_event(15, 0), 5, 16, "tri")
        .expect("valid span")
        .expect("note event");
    assert_eq!(event.whole, None);
    assert_eq!(event.part.start(), &rational(5, 16));
    assert_eq!(event.part.end(), &rational(6, 16));
}

#[test]
fn materialized_cycle_carries_length_spans() {
    // `:` ports east: channel `0`, octave `4`, note `c`, velocity empty
    // (default), length `4`.
    let mut orca = engine(&[".D8.....", "..:04c.4"]);
    let events = materialize_cycle(&mut orca, 8, "tri").expect("materializes");
    assert_eq!(events.len(), 1, "D8 fires only on frame 0 of the cycle");
    assert_eq!(events[0].part.start(), &rational(0, 8));
    assert_eq!(events[0].part.end(), &rational(4, 8));
}

// ---------------------------------------------------------------------------
// `#` comment operator (reference: library.js OperatorComment locks its row
// eastward up to and including the matching `#`, or to the row end).
// ---------------------------------------------------------------------------

#[test]
fn hash_joins_the_grid_alphabet() {
    assert!(is_valid_glyph('#'), "comments are grid glyphs as of v4");
    let orca = engine(&["#ab#"]);
    assert_eq!(rows(&orca), vec!["#ab#"]);
    let mut grid = orpheus_lang::orca::Grid::new(2, 1).expect("valid dimensions");
    assert!(grid.set(0, 0, '#'));
    assert_eq!(grid.glyph_at(0, 0), Some('#'));
}

#[test]
fn commented_operators_do_not_execute() {
    // `D1` bangs below itself every frame when live; inside `#...#` it is
    // locked data and never fires.
    let mut orca = engine(&["#D1#", "...."]);
    orca.tick();
    assert_eq!(rows(&orca), vec!["#D1#", "...."]);
}

#[test]
fn glyphs_after_the_closing_hash_execute() {
    // The comment span ends at the matching `#`; a `D1` east of it is live.
    let mut orca = engine(&["#.#D1.", "......"]);
    orca.tick();
    assert_eq!(rows(&orca), vec!["#.#D1.", "...*.."]);
}

#[test]
fn unmatched_hash_locks_to_the_row_end() {
    let mut orca = engine(&["#..D1", "....."]);
    orca.tick();
    assert_eq!(rows(&orca), vec!["#..D1", "....."]);
}

#[test]
fn comment_only_locks_its_own_row() {
    let mut orca = engine(&["#..#", ".D1.", "...."]);
    orca.tick();
    assert_eq!(rows(&orca), vec!["#..#", ".D1.", ".*.."]);
}

#[test]
fn commented_lowercase_ignores_adjacent_bangs() {
    // A lowercase operator inside a comment is locked, so a bang north of it
    // does not make it run (the free `*` self-erases as usual).
    let mut orca = engine(&[".*..", "#e.#"]);
    orca.tick();
    assert_eq!(rows(&orca), vec!["....", "#e.#"]);
}

#[test]
fn commented_movers_do_not_move() {
    let mut orca = engine(&["#E.#"]);
    orca.tick();
    assert_eq!(rows(&orca), vec!["#E.#"]);
}

#[test]
fn commented_bang_never_self_erases() {
    // The `*` operator inside a comment is locked, so it stays on the grid
    // (in the reference the lock skips its self-erase run).
    let mut orca = engine(&["#*.#"]);
    orca.tick();
    orca.tick();
    assert_eq!(rows(&orca), vec!["#*.#"]);
}

#[test]
fn commented_bang_still_triggers_neighbors_outside_the_comment() {
    // Faithful reference quirk: `hasNeighbor('*')` reads raw glyphs, so a
    // commented (never-erasing) `*` keeps triggering an unlocked lowercase
    // operator on the row below.
    let mut orca = engine(&["#*#", ".e."]);
    orca.tick();
    assert_eq!(rows(&orca), vec!["#*#", "..e"]);
}
