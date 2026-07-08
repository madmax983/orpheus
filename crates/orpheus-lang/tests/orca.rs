//! Behavior tests for the Orca grid engine spike (`orpheus_lang::orca`).
//!
//! Each test pins one of the reference Orca semantics documented in
//! `docs/design/orca-surface.md`.

use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplSession;
use orpheus_lang::orca::{
    DEFAULT_GRID_FRAMES_PER_CYCLE, DEFAULT_SAMPLE_TOKEN, Grid, MidiNote, ORCA_PATTERN_NAME,
    OrcaEngine, OrcaEvent, OrcaIoEvent, OrcaPublisher, frame_span, materialize_cycle,
    playhead_frame, sample_event_from_orca,
};
use orpheus_pattern::Rational;

fn engine(rows: &[&str]) -> OrcaEngine {
    OrcaEngine::from_rows(rows).expect("test grids are well-formed")
}

fn rows(engine: &OrcaEngine) -> Vec<String> {
    engine.grid().rows()
}

#[test]
fn east_moves_one_cell_per_frame() {
    let mut orca = engine(&["E..."]);
    orca.tick();
    assert_eq!(rows(&orca), vec![".E.."], "one tick moves exactly one cell");
    orca.tick();
    assert_eq!(rows(&orca), vec!["..E."]);
}

#[test]
fn all_four_movement_operators_move() {
    let mut orca = engine(&["...", ".N.", "..."]);
    orca.tick();
    assert_eq!(rows(&orca), vec![".N.", "...", "..."]);

    let mut orca = engine(&["...", ".S.", "..."]);
    orca.tick();
    assert_eq!(rows(&orca), vec!["...", "...", ".S."]);

    let mut orca = engine(&["...", ".W.", "..."]);
    orca.tick();
    assert_eq!(rows(&orca), vec!["...", "W..", "..."]);
}

#[test]
fn collision_explodes_to_bang_which_self_erases() {
    // Verified reference sequence: `E.5` -> `.E5` -> `.*5` -> `..5`.
    let mut orca = engine(&["E.5"]);
    orca.tick();
    assert_eq!(rows(&orca), vec![".E5"]);
    orca.tick();
    assert_eq!(rows(&orca), vec![".*5"], "collision replaces mover with *");
    orca.tick();
    assert_eq!(rows(&orca), vec!["..5"], "the explosion bang self-erases");
}

#[test]
fn out_of_bounds_explodes_to_bang() {
    let mut orca = engine(&["..E"]);
    orca.tick();
    assert_eq!(rows(&orca), vec!["..*"], "no wrapping: edge explodes");
    orca.tick();
    assert_eq!(rows(&orca), vec!["..."]);
}

#[test]
fn bang_lasts_exactly_one_frame() {
    let mut orca = engine(&["*"]);
    orca.tick();
    assert_eq!(rows(&orca), vec!["."], "a bang erases itself when scanned");
}

#[test]
fn lowercase_operator_is_inert_without_bang() {
    let mut orca = engine(&["e.."]);
    orca.tick();
    orca.tick();
    assert_eq!(
        rows(&orca),
        vec!["e.."],
        "lone lowercase operator never runs"
    );
}

#[test]
fn lowercase_operator_fires_on_adjacent_bang() {
    // The bang sits south of `e`; the scan reaches `e` first (row 0), sees the
    // bang, and moves. The bang then self-erases in the same frame.
    let mut orca = engine(&["e..", "*.."]);
    orca.tick();
    assert_eq!(rows(&orca), vec![".e.", "..."]);
    orca.tick();
    assert_eq!(rows(&orca), vec![".e.", "..."], "no bang: inert again");
}

#[test]
fn hand_placed_bang_north_of_lowercase_does_not_fire_it() {
    // Scan-order asymmetry (matches orca-js): the orphan bang at row 0 erases
    // itself before the scan reaches `e` at row 1.
    let mut orca = engine(&["*..", "e.."]);
    orca.tick();
    assert_eq!(rows(&orca), vec!["...", "e.."]);
}

#[test]
fn producer_bang_triggers_lowercase_in_same_frame() {
    // `D` (empty rate/mod: bangs every frame) writes a locked `*` below
    // itself; the lowercase `e` east of that cell sees it the same frame.
    let mut orca = engine(&[".D..", "..e."]);
    orca.tick();
    assert_eq!(rows(&orca), vec![".D..", ".*.e"]);
}

#[test]
fn clock_counts_frames_with_rate_and_mod() {
    // `.C4`: rate defaults to 1, mod 4 -> output cycles 0,1,2,3,0.
    let mut orca = engine(&[".C4", "..."]);
    let mut outputs = Vec::new();
    for _ in 0..5 {
        orca.tick();
        outputs.push(orca.grid().glyph_at(1, 1).expect("in bounds"));
    }
    assert_eq!(outputs, vec!['0', '1', '2', '3', '0']);
}

#[test]
fn clock_rate_divides_the_frame_counter() {
    // `2C4`: floor(frame / 2) % 4 -> two frames per step.
    let mut orca = engine(&["2C4", "..."]);
    let mut outputs = Vec::new();
    for _ in 0..6 {
        orca.tick();
        outputs.push(orca.grid().glyph_at(1, 1).expect("in bounds"));
    }
    assert_eq!(outputs, vec!['0', '0', '1', '1', '2', '2']);
}

#[test]
fn delay_bangs_on_its_interval() {
    // `.D2`: rate 1, mod 2 -> bang when frame % 2 == 0, `.` otherwise.
    let mut orca = engine(&[".D2", "..."]);
    let mut outputs = Vec::new();
    for _ in 0..4 {
        orca.tick();
        outputs.push(orca.grid().glyph_at(1, 1).expect("in bounds"));
    }
    assert_eq!(outputs, vec!['*', '.', '*', '.']);
}

#[test]
fn delay_with_mod_one_bangs_every_frame() {
    let mut orca = engine(&[".D1", "..."]);
    for _ in 0..3 {
        orca.tick();
        assert_eq!(orca.grid().glyph_at(1, 1), Some('*'));
    }
}

#[test]
fn add_writes_base36_sum_below() {
    let mut orca = engine(&["1A2", "..."]);
    orca.tick();
    assert_eq!(orca.grid().glyph_at(1, 1), Some('3'));
}

#[test]
fn add_wraps_mod_36() {
    // z (35) + 3 = 38 -> 38 % 36 = 2.
    let mut orca = engine(&["zA3", "..."]);
    orca.tick();
    assert_eq!(orca.grid().glyph_at(1, 1), Some('2'));
}

#[test]
fn add_output_case_follows_right_operand() {
    // Verified reference: `1Ab` -> `c`, `1AB` -> `C`.
    let mut orca = engine(&["1Ab", "..."]);
    orca.tick();
    assert_eq!(orca.grid().glyph_at(1, 1), Some('c'));

    let mut orca = engine(&["1AB", "..."]);
    orca.tick();
    assert_eq!(orca.grid().glyph_at(1, 1), Some('C'));
}

#[test]
fn locked_operand_is_not_executed_in_the_same_frame() {
    // `B` is claimed as `A`'s right operand, so the subtract operator inside
    // it must not run: the cell below `B` stays empty.
    let mut orca = engine(&["1AB.", "...."]);
    orca.tick();
    assert_eq!(
        orca.grid().glyph_at(1, 1),
        Some('C'),
        "A ran: 1 + B(11) = 12 = c, uppercased by the case rule"
    );
    assert_eq!(
        orca.grid().glyph_at(2, 1),
        Some('.'),
        "B was locked as data and must not execute"
    );
}

#[test]
fn moved_operator_is_not_reexecuted_in_the_same_frame() {
    // The destination cell is locked after a move, so a single tick moves the
    // operator exactly once even though the scan passes over its new cell.
    let mut orca = engine(&["E...."]);
    orca.tick();
    assert_eq!(rows(&orca), vec![".E..."], "not \"..E..\"");
}

#[test]
fn output_operator_emits_event_when_banged() {
    // `.D2.` bangs below itself on even frames; the bang cell is west of `:`,
    // whose ports read channel `0`, octave `4`, note `c` (velocity and
    // length fall back to the reference defaults).
    let mut orca = engine(&[".D2...", "..:04c"]);
    let events = orca.tick().to_vec();
    assert_eq!(
        events,
        vec![OrcaEvent {
            frame: 0,
            x: 2,
            y: 1,
            io: OrcaIoEvent::Midi(MidiNote {
                channel: 0,
                octave: 4,
                note: 'c',
                velocity: 15,
                length: 1,
            }),
        }]
    );
    let events = orca.tick().to_vec();
    assert!(events.is_empty(), "no bang on odd frames, no event");
    let events = orca.tick().to_vec();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].frame, 2);
}

#[test]
fn output_operator_locks_its_data_ports() {
    // An uppercase `E` sits in the length port of a banged `:`. The port
    // lock must keep it inert data rather than a moving operator, and the
    // note event must read the explicit velocity (`f` = 15) either way.
    let mut orca = engine(&[".D1.....", "..:04cfE"]);
    orca.tick();
    assert_eq!(
        orca.grid().rows()[1],
        ".*:04cfE",
        "the length glyph is data: E must not move (the `*` is D's bang)"
    );
    assert_eq!(orca.events().len(), 1);
    assert_eq!(
        orca.events()[0].io,
        OrcaIoEvent::Midi(MidiNote {
            channel: 0,
            octave: 4,
            note: 'c',
            velocity: 15,
            length: 14,
        })
    );
}

#[test]
fn frame_span_maps_frames_onto_rational_spans() {
    let span = frame_span(3, 8).expect("valid span");
    assert_eq!(span.start(), &Rational::new(3, 8).expect("valid rational"));
    assert_eq!(span.end(), &Rational::new(1, 2).expect("valid rational"));

    let span = frame_span(0, 8).expect("valid span");
    assert_eq!(span.start(), &Rational::zero());
    assert_eq!(span.end(), &Rational::new(1, 8).expect("valid rational"));

    // Frames past the first cycle keep counting in absolute rational time.
    let span = frame_span(8, 8).expect("valid span");
    assert_eq!(span.start(), &Rational::one());
    assert_eq!(span.end(), &Rational::new(9, 8).expect("valid rational"));
}

#[test]
fn frame_span_rejects_zero_frames_per_cycle() {
    assert!(frame_span(0, 0).is_err());
}

#[test]
fn frame_counter_advances_per_tick() {
    let mut orca = engine(&["..."]);
    assert_eq!(orca.frame(), 0);
    orca.tick();
    orca.tick();
    assert_eq!(orca.frame(), 2);
}

#[test]
fn grid_edit_between_ticks_takes_effect() {
    let mut orca = engine(&["...."]);
    assert!(orca.grid_mut().set(0, 0, 'E'));
    orca.tick();
    assert_eq!(rows(&orca), vec![".E.."]);
}

#[test]
fn grid_rejects_malformed_input() {
    assert!(Grid::from_rows(&["ab", "abc"]).is_err());
    assert!(Grid::from_rows(&[".#."]).is_err());
    assert!(Grid::from_rows(&[]).is_err());
}

// ---------------------------------------------------------------------------
// Publish bridge (v1): OrcaEvent -> Event<SampleEvent> -> session publication.
// ---------------------------------------------------------------------------

const fn note_event(octave: u8, note: char) -> OrcaEvent {
    OrcaEvent {
        frame: 3,
        x: 2,
        y: 1,
        io: OrcaIoEvent::Midi(MidiNote {
            channel: 0,
            octave,
            note,
            velocity: 15,
            length: 1,
        }),
    }
}

#[test]
fn orca_event_converts_to_sample_event_with_frame_span() {
    let orca_event = note_event(4, 'C');
    let event = sample_event_from_orca(&orca_event, 3, 16, DEFAULT_SAMPLE_TOKEN)
        .expect("valid frame span")
        .expect("note events map to sample events");

    assert_eq!(event.whole, None, "unclipped unit-cycle event");
    assert_eq!(event.part.start(), &Rational::new(3, 16).expect("rational"));
    assert_eq!(event.part.end(), &Rational::new(4, 16).expect("rational"));
    assert_eq!(event.value.sample(), DEFAULT_SAMPLE_TOKEN);
    // Octave 4 C is MIDI 72, +12 semitones above middle C: one octave up.
    assert!((event.value.rate() - 2.0).abs() < 1e-12);
}

#[test]
fn orca_event_at_middle_c_keeps_base_rate() {
    let orca_event = note_event(3, 'C');
    let event = sample_event_from_orca(&orca_event, 0, 16, "bd")
        .expect("valid frame span")
        .expect("note events map to sample events");
    assert_eq!(event.value.sample(), "bd");
    assert!((event.value.rate() - 1.0).abs() < 1e-12);
}

#[test]
fn orca_event_conversion_rejects_zero_frames_per_cycle() {
    let orca_event = note_event(3, 'C');
    assert!(sample_event_from_orca(&orca_event, 0, 0, "bd").is_err());
}

#[test]
fn materialize_cycle_stamps_ordered_frame_spans() {
    // `.D4.` bangs below itself when frame % 4 == 0; `:` east of the bang cell
    // emits note `c`. Over one 8-frame cycle: events at frames 0 and 4.
    let mut orca = engine(&[".D4...", "..:04c"]);
    let events = materialize_cycle(&mut orca, 8, DEFAULT_SAMPLE_TOKEN).expect("materializes");

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(
        events[0].part.end(),
        &Rational::new(1, 8).expect("rational")
    );
    assert_eq!(
        events[1].part.start(),
        &Rational::new(4, 8).expect("rational")
    );
    assert_eq!(
        events[1].part.end(),
        &Rational::new(5, 8).expect("rational")
    );
    assert!(
        events
            .windows(2)
            .all(|pair| pair[0].part.start() <= pair[1].part.start()),
        "events are ordered by span start"
    );
}

#[test]
fn materialize_cycle_supports_multiple_events_per_frame() {
    // Two independent `D1` operators bang every frame; each feeds its own `:`.
    let mut orca = engine(&[".D1......D1...", "..:04C....:04c"]);
    let events = materialize_cycle(&mut orca, 2, DEFAULT_SAMPLE_TOKEN).expect("materializes");

    assert_eq!(events.len(), 4, "two events per frame over two frames");
    assert_eq!(
        events[0].part, events[1].part,
        "same-frame events share a span"
    );
    assert_eq!(
        events[2].part.start(),
        &Rational::new(1, 2).expect("rational")
    );
    // Scan order within a frame: `:04C` (west, octave 4 C = +12 semitones)
    // before `:04c` (east, octave 4 C# = +13 semitones).
    assert!((events[0].value.rate() - 2.0).abs() < 1e-12);
    assert!((events[1].value.rate() - (13.0 / 12.0_f64).exp2()).abs() < 1e-12);
}

#[test]
fn materialize_cycle_advances_grid_state_across_calls() {
    // A running grid is not cycle-periodic: `E` keeps moving east, so cycle 2
    // starts from where cycle 1 left the grid.
    let mut orca = engine(&["E......."]);
    materialize_cycle(&mut orca, 3, DEFAULT_SAMPLE_TOKEN).expect("materializes");
    assert_eq!(orca.grid().rows(), vec!["...E....".to_owned()]);
    materialize_cycle(&mut orca, 3, DEFAULT_SAMPLE_TOKEN).expect("materializes");
    assert_eq!(orca.grid().rows(), vec!["......E.".to_owned()]);
}

#[test]
fn publisher_republishes_at_each_cycle_boundary() {
    // `.D8.` bangs on frames 0, 8, 16, ... With 4 grid frames per cycle the
    // grid is not cycle-periodic: cycle 0 fires, cycle 1 is silent, cycle 2
    // fires again. Each engine cycle boundary must produce a fresh batch.
    let orca = engine(&[".D8...", "..:04c"]);
    let mut publisher = OrcaPublisher::new(orca, 4, DEFAULT_SAMPLE_TOKEN);

    assert!(
        publisher.poll(0).expect("materializes").is_none(),
        "a stopped publisher never publishes"
    );

    publisher.start();
    let cycle0 = publisher
        .poll(0)
        .expect("materializes")
        .expect("first poll publishes cycle 0");
    assert_eq!(cycle0.len(), 1, "frame 0 fires in cycle 0");

    assert!(
        publisher.poll(0).expect("materializes").is_none(),
        "same cycle boundary: no re-publish"
    );

    let cycle1 = publisher
        .poll(44_100)
        .expect("materializes")
        .expect("new cycle boundary publishes cycle 1");
    assert!(cycle1.is_empty(), "frames 4..8 are silent");

    let cycle2 = publisher
        .poll(88_200)
        .expect("materializes")
        .expect("new cycle boundary publishes cycle 2");
    assert_eq!(cycle2.len(), 1, "frame 8 fires in cycle 2");
}

#[test]
fn publisher_stop_halts_and_restart_republishes() {
    let orca = engine(&[".D1...", "..:04c"]);
    let mut publisher = OrcaPublisher::new(orca, 2, DEFAULT_SAMPLE_TOKEN);
    publisher.start();
    assert!(publisher.is_running());
    assert!(publisher.poll(0).expect("materializes").is_some());

    publisher.stop();
    assert!(!publisher.is_running());
    assert!(publisher.poll(44_100).expect("materializes").is_none());

    publisher.start();
    assert!(
        publisher.poll(44_100).expect("materializes").is_some(),
        "restart forgets the last boundary and publishes immediately"
    );
}

#[test]
fn publisher_default_grid_matches_documented_dimensions() {
    let publisher = OrcaPublisher::with_default_grid();
    assert_eq!(publisher.engine().grid().width(), 16);
    assert_eq!(publisher.engine().grid().height(), 8);
    assert_eq!(publisher.frames_per_cycle(), DEFAULT_GRID_FRAMES_PER_CYCLE);
    assert!(!publisher.is_running());
}

#[test]
fn playhead_frame_maps_engine_position_to_grid_frame() {
    // Halfway through a 44100-frame cycle with 16 grid frames -> frame 8.
    assert_eq!(playhead_frame(22_050, 0, 44_100, 16), Some(8));
    assert_eq!(playhead_frame(0, 0, 44_100, 16), Some(0));
    // Mid-stream: offsets are relative to the current cycle start.
    assert_eq!(playhead_frame(88_200 + 22_050, 88_200, 44_100, 16), Some(8));
    // The playhead clamps to the final frame at the cycle's last sample.
    assert_eq!(playhead_frame(44_099, 0, 44_100, 16), Some(15));
    assert_eq!(playhead_frame(44_100 + 7, 0, 44_100, 16), Some(15));
    // Degenerate clocks report no playhead.
    assert_eq!(playhead_frame(10, 0, 0, 16), None);
    assert_eq!(playhead_frame(10, 0, 44_100, 0), None);
}

#[test]
fn session_publishes_materialized_grid_cycle_as_binding() {
    let mut session = ReplSession::with_engine(EngineHandle::stub());
    let mut orca = engine(&[".D1...", "..:04c"]);
    let events = materialize_cycle(&mut orca, 4, DEFAULT_SAMPLE_TOKEN).expect("materializes");
    assert_eq!(events.len(), 4);

    session
        .publish_sample_events(ORCA_PATTERN_NAME, events)
        .expect("publishes to the stub engine");
    assert!(
        session
            .binding_summaries()
            .iter()
            .any(|summary| summary == "orca: Pattern<Sample>"),
        "the grid publishes under a session binding"
    );

    // Re-publishing the next cycle under the same name must also succeed.
    let next_cycle = materialize_cycle(&mut orca, 4, DEFAULT_SAMPLE_TOKEN).expect("materializes");
    session
        .publish_sample_events(ORCA_PATTERN_NAME, next_cycle)
        .expect("re-publishes at the cycle boundary");
}
