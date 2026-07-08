//! Behavior tests for the Orca grid engine spike (`orpheus_lang::orca`).
//!
//! Each test pins one of the reference Orca semantics documented in
//! `docs/design/orca-surface.md`.

use orpheus_lang::orca::{Grid, OrcaEngine, OrcaEvent, frame_span};
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
    // whose note port holds `c` (base-36 value 12).
    let mut orca = engine(&[".D2.", "..:c"]);
    let events = orca.tick().to_vec();
    assert_eq!(
        events,
        vec![OrcaEvent {
            frame: 0,
            x: 2,
            y: 1,
            note: 'c',
            value: 12,
        }]
    );
    let events = orca.tick().to_vec();
    assert!(events.is_empty(), "no bang on odd frames, no event");
    let events = orca.tick().to_vec();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].frame, 2);
}

#[test]
fn output_operator_locks_its_note_port() {
    // The note port holds a lowercase `d` (delay) next to a bang; the port
    // lock must keep it inert data rather than a running operator.
    let mut orca = engine(&[".D1.", "..:d", "...."]);
    orca.tick();
    assert_eq!(
        orca.grid().glyph_at(3, 2),
        Some('.'),
        "the note glyph is data: it must not bang below itself"
    );
    assert_eq!(orca.events().len(), 1);
    assert_eq!(orca.events()[0].note, 'd');
    assert_eq!(orca.events()[0].value, 13);
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
