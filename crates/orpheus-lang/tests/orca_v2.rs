//! Behavior tests for the v2 Orca operator set (`orpheus_lang::orca`).
//!
//! Each test pins reference orca-js semantics (hundredrabbits/Orca, branch
//! `main`, `library.js`/`operator.js`) as documented in the v2 section of
//! `docs/design/orca-surface.md`: port offsets, locking behavior,
//! case-sensitive outputs, and bang-vs-frame execution.

use orpheus_lang::OrcaEngine;

fn engine(rows: &[&str]) -> OrcaEngine {
    OrcaEngine::from_rows(rows).expect("test grids are well-formed")
}

fn glyph(engine: &OrcaEngine, x: usize, y: usize) -> char {
    engine.grid().glyph_at(x, y).expect("in bounds")
}

/// Ticks once and returns the glyph below the given cell.
fn tick_below(orca: &mut OrcaEngine, x: usize, y: usize) -> char {
    orca.tick();
    glyph(orca, x, y + 1)
}

// ---------------------------------------------------------------------------
// R (random): inclusive range, deterministic replay from (frame, position).
// ---------------------------------------------------------------------------

#[test]
fn random_outputs_stay_in_inclusive_range() {
    let mut orca = engine(&["2R5", "..."]);
    for _ in 0..32 {
        let out = tick_below(&mut orca, 1, 0);
        assert!(
            ('2'..='5').contains(&out),
            "R output {out:?} must lie in the inclusive range [2, 5]"
        );
    }
}

#[test]
fn random_varies_across_frames() {
    let mut orca = engine(&["0R9", "..."]);
    let outputs: Vec<char> = (0..32).map(|_| tick_below(&mut orca, 1, 0)).collect();
    let first = outputs[0];
    assert!(
        outputs.iter().any(|&out| out != first),
        "a [0,9] range must produce more than one value over 32 frames"
    );
}

#[test]
fn random_with_equal_operands_outputs_the_operand() {
    let mut orca = engine(&["4R4", "..."]);
    for _ in 0..8 {
        assert_eq!(tick_below(&mut orca, 1, 0), '4');
    }
}

#[test]
fn random_with_swapped_operands_uses_the_same_range() {
    // orca-js main swaps when a > b: `5R2` draws from [2, 5] inclusive.
    let mut orca = engine(&["5R2", "..."]);
    for _ in 0..32 {
        let out = tick_below(&mut orca, 1, 0);
        assert!(
            ('2'..='5').contains(&out),
            "R output {out:?} must lie in the inclusive range [2, 5]"
        );
    }
}

#[test]
fn random_is_deterministic_and_replayable() {
    let mut first = engine(&["0Rz", "..."]);
    let mut second = engine(&["0Rz", "..."]);
    for frame in 0..24 {
        assert_eq!(
            tick_below(&mut first, 1, 0),
            tick_below(&mut second, 1, 0),
            "frame {frame}: identical grids must replay identical R outputs"
        );
    }
}

#[test]
fn random_streams_differ_by_grid_position() {
    // Two Rs with identical operands: the seed hashes the operator position,
    // so the two output streams must not be identical.
    let mut orca = engine(&["0Rz.0Rz", "......."]);
    let mut west = Vec::new();
    let mut east = Vec::new();
    for _ in 0..24 {
        orca.tick();
        west.push(glyph(&orca, 1, 1));
        east.push(glyph(&orca, 5, 1));
    }
    assert_ne!(west, east, "position must decorrelate R streams");
}

#[test]
fn random_output_case_follows_right_operand() {
    let mut orca = engine(&["aRZ", "..."]);
    let out = tick_below(&mut orca, 1, 0);
    assert!(
        out.is_ascii_uppercase(),
        "uppercase right operand must uppercase the output, got {out:?}"
    );

    let mut orca = engine(&["aRz", "..."]);
    let out = tick_below(&mut orca, 1, 0);
    assert!(
        out.is_ascii_lowercase(),
        "lowercase right operand keeps the output lowercase, got {out:?}"
    );
}

// ---------------------------------------------------------------------------
// I (increment): reads its own output cell as state, steps by rate, mod wrap.
// ---------------------------------------------------------------------------

#[test]
fn increment_steps_and_wraps_mod() {
    let mut orca = engine(&["1I3", "..."]);
    let outputs: Vec<char> = (0..6).map(|_| tick_below(&mut orca, 1, 0)).collect();
    assert_eq!(outputs, vec!['1', '2', '0', '1', '2', '0']);
}

#[test]
fn increment_reads_preexisting_state_from_the_grid() {
    let mut orca = engine(&["1I5", ".4."]);
    assert_eq!(tick_below(&mut orca, 1, 0), '0', "(4 + 1) mod 5 = 0");
}

#[test]
fn increment_empty_step_means_step_zero() {
    // Reference: no default step in main-branch orca-js — empty step is +0.
    let mut orca = engine(&[".I8", "..."]);
    orca.tick();
    orca.tick();
    assert_eq!(glyph(&orca, 1, 1), '0', "(0 + 0) mod 8 stays 0");
}

#[test]
fn increment_empty_mod_outputs_zero() {
    // Reference: `mod ? keyOf((val + step) % mod) : '0'`.
    let mut orca = engine(&["1I.", "..."]);
    for _ in 0..3 {
        assert_eq!(tick_below(&mut orca, 1, 0), '0');
    }
}

#[test]
fn increment_output_case_follows_right_operand() {
    // step a=10, mod Z=35: (0 + 10) mod 35 = 10 -> 'a', uppercased by 'Z'.
    let mut orca = engine(&["aIZ", "..."]);
    assert_eq!(tick_below(&mut orca, 1, 0), 'A');
    assert_eq!(
        tick_below(&mut orca, 1, 0),
        'K',
        "(10 + 10) mod 35 = 20 -> K"
    );
}

#[test]
fn increment_output_cell_is_locked_against_execution() {
    // With mod z (35) and step b (11), frame 0 writes 'b' below. A locked
    // output must not execute in the frame it was produced, and I rewrites
    // (and re-locks) it every subsequent frame.
    let mut orca = engine(&["bIz", "...", "..."]);
    orca.tick();
    assert_eq!(glyph(&orca, 1, 1), 'b');
    orca.tick();
    assert_eq!(glyph(&orca, 1, 1), 'm', "state advanced: (11 + 11) mod 35");
    assert_eq!(glyph(&orca, 1, 2), '.', "the written 'b' never executed");
}

// ---------------------------------------------------------------------------
// V (variable): write form, read form, per-frame reset, scan-order dependency.
// ---------------------------------------------------------------------------

#[test]
fn variable_write_then_read_in_scan_order() {
    let mut orca = engine(&["aV5..", ".....", ".Va..", "....."]);
    orca.tick();
    assert_eq!(
        glyph(&orca, 1, 3),
        '5',
        "reader below outputs the stored value"
    );
}

#[test]
fn variable_write_form_produces_no_output() {
    let mut orca = engine(&["aV5", "..."]);
    orca.tick();
    assert_eq!(glyph(&orca, 1, 1), '.', "write form must not write below");
}

#[test]
fn variables_reset_every_frame_so_readers_must_follow_writers() {
    // The reader sits above the writer, i.e. earlier in scan order. If
    // variables persisted across frames, frame 1 would read frame 0's write.
    let mut orca = engine(&[".Va..", ".....", "aV5..", "....."]);
    orca.tick();
    orca.tick();
    assert_eq!(
        glyph(&orca, 1, 1),
        '.',
        "variables are cleared at frame start: the earlier reader sees nothing"
    );
}

#[test]
fn variable_read_copies_the_stored_glyph_verbatim() {
    let mut orca = engine(&["bVC..", ".....", ".Vb..", "....."]);
    orca.tick();
    assert_eq!(
        glyph(&orca, 1, 3),
        'C',
        "V output is not case-sensitive: verbatim copy"
    );
}

#[test]
fn variable_names_are_case_sensitive() {
    let mut orca = engine(&["aV5..", ".....", ".VA..", "....."]);
    orca.tick();
    assert_eq!(
        glyph(&orca, 1, 3),
        '.',
        "'A' names a different variable than 'a'"
    );
}

// ---------------------------------------------------------------------------
// B (subtract), L (lesser), M (multiply): arithmetic with the case rule.
// ---------------------------------------------------------------------------

#[test]
fn subtract_outputs_absolute_difference() {
    let mut orca = engine(&["1B3", "..."]);
    assert_eq!(tick_below(&mut orca, 1, 0), '2');
    let mut orca = engine(&["3B1", "..."]);
    assert_eq!(tick_below(&mut orca, 1, 0), '2');
}

#[test]
fn subtract_output_case_follows_right_operand() {
    let mut orca = engine(&["1BC", "..."]);
    assert_eq!(
        tick_below(&mut orca, 1, 0),
        'B',
        "|12 - 1| = 11 -> b, uppercased"
    );
}

#[test]
fn lesser_outputs_the_smaller_value() {
    let mut orca = engine(&["3L5", "..."]);
    assert_eq!(tick_below(&mut orca, 1, 0), '3');
    let mut orca = engine(&["5L3", "..."]);
    assert_eq!(tick_below(&mut orca, 1, 0), '3');
}

#[test]
fn lesser_output_case_follows_right_operand() {
    let mut orca = engine(&["bLC", "..."]);
    assert_eq!(
        tick_below(&mut orca, 1, 0),
        'B',
        "min(11, 12) = 11 -> b, uppercased"
    );
}

#[test]
fn multiply_outputs_product_mod_36() {
    let mut orca = engine(&["3M4", "..."]);
    assert_eq!(tick_below(&mut orca, 1, 0), 'c', "3 * 4 = 12 -> c");
    let mut orca = engine(&["7M6", "..."]);
    assert_eq!(tick_below(&mut orca, 1, 0), '6', "42 mod 36 = 6");
}

#[test]
fn multiply_output_case_follows_right_operand() {
    let mut orca = engine(&["4MC", "..."]);
    assert_eq!(
        tick_below(&mut orca, 1, 0),
        'C',
        "4 * 12 = 48 mod 36 = 12 -> c, uppercased"
    );
}

// ---------------------------------------------------------------------------
// F (if): bangs below on glyph equality (case-sensitive comparison).
// ---------------------------------------------------------------------------

#[test]
fn if_bangs_below_on_equal_glyphs() {
    let mut orca = engine(&["3F3", "..."]);
    assert_eq!(tick_below(&mut orca, 1, 0), '*');
}

#[test]
fn if_writes_empty_below_on_unequal_glyphs() {
    let mut orca = engine(&["3F4", "..."]);
    assert_eq!(tick_below(&mut orca, 1, 0), '.');
}

#[test]
fn if_comparison_is_case_sensitive() {
    let mut orca = engine(&["aFA", "..."]);
    assert_eq!(
        tick_below(&mut orca, 1, 0),
        '.',
        "'a' and 'A' are different glyphs"
    );
}

#[test]
fn if_empty_operands_compare_equal() {
    let mut orca = engine(&[".F.", "..."]);
    assert_eq!(tick_below(&mut orca, 1, 0), '*', "'.' == '.' bangs");
}

#[test]
fn if_bang_triggers_lowercase_neighbor_in_the_same_frame() {
    // F's bang output is locked (like D's), so the lowercase `e` east of the
    // bang cell fires the same frame.
    let mut orca = engine(&["3F3..", "..e.."]);
    orca.tick();
    assert_eq!(glyph(&orca, 3, 1), 'e', "e moved east off the bang");
}

// ---------------------------------------------------------------------------
// G (generate): writes its eastward operands to an offset block.
// ---------------------------------------------------------------------------

#[test]
fn generate_copies_operands_below_by_default() {
    // x/y empty (0), len 2: reads the two glyphs east of G, writes them to
    // the row below starting directly under G.
    let mut orca = engine(&["..2Gab", "......"]);
    orca.tick();
    assert_eq!(glyph(&orca, 3, 1), 'a');
    assert_eq!(glyph(&orca, 4, 1), 'b');
}

#[test]
fn generate_writes_at_the_given_offset() {
    // x=1, y=2, len=1: the operand lands at relative {+1, +3}.
    let mut orca = engine(&["121Gz.", "......", "......", "......"]);
    orca.tick();
    assert_eq!(glyph(&orca, 4, 3), 'z');
}

#[test]
fn generate_locks_operands_and_written_cells() {
    // The copied `E` is locked where it lands, and the operand `E` is locked
    // as data: neither executes, this frame or (G re-locks) any later frame.
    let mut orca = engine(&["..1GE", "....."]);
    orca.tick();
    assert_eq!(glyph(&orca, 3, 1), 'E', "copy written below G");
    assert_eq!(glyph(&orca, 4, 0), 'E', "operand did not move");
    orca.tick();
    assert_eq!(
        glyph(&orca, 3, 1),
        'E',
        "G re-locks its output block every frame"
    );
    assert_eq!(glyph(&orca, 4, 0), 'E', "operand stays locked every frame");
}

// ---------------------------------------------------------------------------
// H (halt): locks the cell below, writes nothing.
// ---------------------------------------------------------------------------

#[test]
fn halt_freezes_the_operator_below_indefinitely() {
    let mut orca = engine(&["H..", "E..", "..."]);
    for _ in 0..3 {
        orca.tick();
        assert_eq!(glyph(&orca, 0, 1), 'E', "E under H never moves");
    }
}

#[test]
fn halt_does_not_write_into_the_halted_cell() {
    let mut orca = engine(&["H..", "...", "..."]);
    orca.tick();
    assert_eq!(glyph(&orca, 0, 1), '.', "H makes no write of its own");
}

// ---------------------------------------------------------------------------
// J (jumper) and Y (jymper): value teleport across operator chains.
// ---------------------------------------------------------------------------

#[test]
fn jumper_copies_the_northward_value_below() {
    let mut orca = engine(&["5..", "J..", "..."]);
    orca.tick();
    assert_eq!(glyph(&orca, 0, 2), '5');
    assert_eq!(
        glyph(&orca, 0, 0),
        '5',
        "the source value is copied, not moved"
    );
}

#[test]
fn jumper_chain_teleports_in_one_frame() {
    // Only the head of a J column runs (a J with J above is dormant); the
    // head writes past the whole chain in a single frame.
    let mut orca = engine(&["5", "J", "J", "."]);
    orca.tick();
    assert_eq!(glyph(&orca, 0, 3), '5');
    assert_eq!(glyph(&orca, 0, 2), 'J', "chain body is untouched");
}

#[test]
fn jumper_copies_empty_cells_too() {
    let mut orca = engine(&[".", "J", "5"]);
    orca.tick();
    assert_eq!(
        glyph(&orca, 0, 2),
        '.',
        "an empty north clears the output cell"
    );
}

#[test]
fn jymper_copies_the_westward_value_east() {
    let mut orca = engine(&["5Y.."]);
    orca.tick();
    assert_eq!(glyph(&orca, 2, 0), '5');
}

#[test]
fn jymper_chain_teleports_in_one_frame() {
    let mut orca = engine(&["5YY."]);
    orca.tick();
    assert_eq!(glyph(&orca, 3, 0), '5');
    assert_eq!(glyph(&orca, 2, 0), 'Y', "chain body is untouched");
}

// ---------------------------------------------------------------------------
// K (konkat): reads multiple variables beneath their keys.
// ---------------------------------------------------------------------------

#[test]
fn konkat_writes_variable_values_beneath_their_keys() {
    let mut orca = engine(&["aV5..", ".....", ".2Kab", "....."]);
    orca.tick();
    assert_eq!(glyph(&orca, 3, 3), '5', "variable a resolves under its key");
    assert_eq!(
        glyph(&orca, 4, 3),
        '.',
        "unset variable b resolves to empty"
    );
}

#[test]
fn konkat_locks_its_key_cells() {
    // The uppercase E east of K is claimed as a key, not executed.
    let mut orca = engine(&["1KE..", "....."]);
    orca.tick();
    assert_eq!(glyph(&orca, 2, 0), 'E', "key cell is locked data");
}

// ---------------------------------------------------------------------------
// O (read): reads a glyph at an offset and outputs it below.
// ---------------------------------------------------------------------------

#[test]
fn read_defaults_to_the_east_neighbor() {
    // x/y empty: reads relative {0+1, 0}, the cell east of O.
    let mut orca = engine(&["..O5.", "....."]);
    orca.tick();
    assert_eq!(glyph(&orca, 2, 1), '5');
}

#[test]
fn read_uses_x_and_y_offsets() {
    // x=2, y=1: reads relative {3, 1}.
    let mut orca = engine(&["21O......", ".....z..."]);
    orca.tick();
    assert_eq!(glyph(&orca, 2, 1), 'z');
}

// ---------------------------------------------------------------------------
// P (push): writes its value into a slot of the row below.
// ---------------------------------------------------------------------------

#[test]
fn push_writes_the_value_into_slot_key_mod_len() {
    // key=1, len=2, val=5: slot 1 of the two-cell row below P.
    let mut orca = engine(&["12P5.", "....."]);
    orca.tick();
    assert_eq!(glyph(&orca, 3, 1), '5');
    assert_eq!(glyph(&orca, 2, 1), '.', "slot 0 untouched");
}

#[test]
fn push_locks_its_whole_output_row() {
    // The E parked in P's output row is locked and never moves.
    let mut orca = engine(&["12P5.", "..E.."]);
    orca.tick();
    assert_eq!(glyph(&orca, 2, 1), 'E', "output row cell is locked");
    assert_eq!(glyph(&orca, 3, 1), '5');
}

// ---------------------------------------------------------------------------
// Q (query): reads a block at an offset, writes it ending below Q.
// ---------------------------------------------------------------------------

#[test]
fn query_copies_glyphs_ending_directly_below() {
    // x/y empty, len 2: reads the two cells east of Q, writes them so the
    // last lands directly below Q.
    let mut orca = engine(&["..2Qab", "......"]);
    orca.tick();
    assert_eq!(glyph(&orca, 2, 1), 'a');
    assert_eq!(glyph(&orca, 3, 1), 'b');
}

#[test]
fn query_reads_at_the_given_offset() {
    // x=1, y=1, len=1: reads relative {2, 1}.
    let mut orca = engine(&["111Q..", ".....z"]);
    orca.tick();
    assert_eq!(glyph(&orca, 3, 1), 'z');
}

// ---------------------------------------------------------------------------
// T (track): outputs the key-th of its eastward operands.
// ---------------------------------------------------------------------------

#[test]
fn track_outputs_the_indexed_operand() {
    // key=1, len=3, track cells a b c: outputs cell (1 mod 3) = b.
    let mut orca = engine(&["13Tabc", "......"]);
    orca.tick();
    assert_eq!(glyph(&orca, 2, 1), 'b');
}

#[test]
fn track_wraps_the_key_by_len() {
    // key=5, len=3: 5 mod 3 = 2 -> c.
    let mut orca = engine(&["53Tabc", "......"]);
    orca.tick();
    assert_eq!(glyph(&orca, 2, 1), 'c');
}

#[test]
fn track_copies_the_operand_verbatim_and_locks_track_cells() {
    // Track cells are locked data (the uppercase E does not run) and the
    // output preserves the operand's case (T's output is not sensitive).
    let mut orca = engine(&["11TE.", "....."]);
    orca.tick();
    assert_eq!(glyph(&orca, 3, 0), 'E', "track cell is locked data");
    assert_eq!(glyph(&orca, 2, 1), 'E', "verbatim copy, case preserved");
}

// ---------------------------------------------------------------------------
// U (uclid): bangs on a Euclidean rhythm.
// ---------------------------------------------------------------------------

#[test]
fn uclid_bangs_euclidean_pattern() {
    // step=3, max=8: bucket = (3 * (f + 7)) % 8 + 3 >= 8, i.e. E(3, 8)
    // starting at frame 0: X..X..X. over the first 8 frames.
    let mut orca = engine(&["3U8", "..."]);
    let outputs: Vec<char> = (0..8).map(|_| tick_below(&mut orca, 1, 0)).collect();
    assert_eq!(outputs, vec!['*', '.', '.', '*', '.', '.', '*', '.']);
}

#[test]
fn uclid_with_empty_step_never_bangs() {
    // No port defaults in main-branch orca-js: empty step clamps to 0.
    let mut orca = engine(&[".U8", "..."]);
    for _ in 0..8 {
        assert_eq!(tick_below(&mut orca, 1, 0), '.');
    }
}

// ---------------------------------------------------------------------------
// X (write): writes its value at an offset.
// ---------------------------------------------------------------------------

#[test]
fn write_places_the_value_at_the_offset() {
    // x=2, y=1: writes at relative {2, 2}.
    let mut orca = engine(&["21X5.", ".....", "....."]);
    orca.tick();
    assert_eq!(glyph(&orca, 4, 2), '5');
}

#[test]
fn write_defaults_to_directly_below() {
    // x/y empty: writes at relative {0, 1}.
    let mut orca = engine(&["..X5.", "....."]);
    orca.tick();
    assert_eq!(glyph(&orca, 2, 1), '5');
}

// ---------------------------------------------------------------------------
// Z (lerp): steps its own output toward the target by rate per frame.
// ---------------------------------------------------------------------------

#[test]
fn lerp_steps_toward_the_target_and_holds() {
    let mut orca = engine(&["1Z5", "..."]);
    let outputs: Vec<char> = (0..6).map(|_| tick_below(&mut orca, 1, 0)).collect();
    assert_eq!(outputs, vec!['1', '2', '3', '4', '5', '5']);
}

#[test]
fn lerp_steps_downward_from_preexisting_state() {
    let mut orca = engine(&["1Z0", ".5."]);
    let outputs: Vec<char> = (0..6).map(|_| tick_below(&mut orca, 1, 0)).collect();
    assert_eq!(outputs, vec!['4', '3', '2', '1', '0', '0']);
}

#[test]
fn lerp_with_empty_rate_holds_its_value() {
    // No rate default in main-branch orca-js: empty rate steps by 0.
    let mut orca = engine(&[".Z5", ".3."]);
    orca.tick();
    orca.tick();
    assert_eq!(glyph(&orca, 1, 1), '3', "rate 0 never moves the value");
}

#[test]
fn lerp_output_case_follows_right_operand() {
    // val b=11 stepping toward C=12 at rate 1 -> 12 -> 'c', uppercased.
    let mut orca = engine(&["1ZC", ".b."]);
    assert_eq!(tick_below(&mut orca, 1, 0), 'C');
}
