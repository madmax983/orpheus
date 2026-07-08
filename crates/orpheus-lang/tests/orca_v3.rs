//! Behavior tests for the Orca v3 IO operator family (`:` `%` `!` `?` `;`
//! `=` `$`).
//!
//! Semantics are pinned against main-branch `hundredrabbits/Orca`
//! (`library.js`, `operator.js`, `io/midi.js`, `transpose.js`): the IO
//! operators are always passive but act only with a `*` in a cardinal
//! neighbor cell, lock their data ports eastward on every frame they run
//! (banged or not), and emit typed events instead of driving real
//! transports. See `docs/design/orca-surface.md` section 9.

use orpheus_lang::orca::{
    MidiNote, OrcaEngine, OrcaEvent, OrcaIoEvent, is_valid_glyph, materialize_cycle, midi_note_id,
    sample_event_from_orca,
};

fn engine(rows: &[&str]) -> OrcaEngine {
    OrcaEngine::from_rows(rows).expect("test grids are well-formed")
}

fn tick_events(engine: &mut OrcaEngine) -> Vec<OrcaEvent> {
    engine.tick().to_vec()
}

const fn midi_event(io: OrcaIoEvent) -> OrcaEvent {
    OrcaEvent {
        frame: 0,
        x: 0,
        y: 0,
        io,
    }
}

// ---------------------------------------------------------------------------
// `:` (MIDI note) — full reference port layout.
// ---------------------------------------------------------------------------

#[test]
fn midi_emits_full_port_event_with_defaults() {
    // `.D1` bangs below itself every frame; the bang cell is west of `:`.
    // Ports east of `:`: channel `0`, octave `4`, note `c`; velocity and
    // length are empty, so the reference defaults apply (`f` = 15, `1` = 1).
    let mut orca = engine(&[".D1...", "..:04c"]);
    let events = tick_events(&mut orca);
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
}

#[test]
fn midi_reads_explicit_velocity_and_length_with_clamps() {
    // Velocity `z` (35) clamps to 16; length `z` (35) clamps to 32.
    let mut orca = engine(&[".D1.....", "..:04czz"]);
    let events = tick_events(&mut orca);
    assert_eq!(events.len(), 1);
    let OrcaIoEvent::Midi(note) = &events[0].io else {
        panic!("expected a Midi event, got {:?}", events[0].io);
    };
    assert_eq!(note.velocity, 16, "velocity clamps to 0-16");
    assert_eq!(note.length, 32, "length clamps to 0-32");
}

#[test]
fn midi_requires_bang_neighbor() {
    // IO operators are passive-but-banged: without a `*` in a cardinal
    // neighbor they never emit, even though they run (and lock) every frame.
    let mut orca = engine(&["..:04c"]);
    assert!(tick_events(&mut orca).is_empty());
    assert!(tick_events(&mut orca).is_empty());
}

#[test]
fn midi_aborts_on_missing_channel_octave_or_note() {
    for rows in [
        &[".D1...", "..:.4c"], // channel empty
        &[".D1...", "..:0.c"], // octave empty
        &[".D1...", "..:04."], // note empty
    ] {
        let mut orca = engine(rows);
        assert!(
            tick_events(&mut orca).is_empty(),
            "grid {rows:?} must not emit"
        );
    }
}

#[test]
fn midi_aborts_on_digit_note() {
    // The reference rejects numeric note glyphs (`isNaN` check).
    let mut orca = engine(&[".D1...", "..:045"]);
    assert!(tick_events(&mut orca).is_empty());
}

#[test]
fn midi_aborts_on_channel_above_15() {
    // `g` is 16: the reference returns without emitting.
    let mut orca = engine(&[".D1...", "..:g4c"]);
    assert!(tick_events(&mut orca).is_empty());
}

#[test]
fn midi_octave_clamps_to_8() {
    let mut orca = engine(&[".D1...", "..:0zc"]);
    let events = tick_events(&mut orca);
    let OrcaIoEvent::Midi(note) = &events[0].io else {
        panic!("expected a Midi event, got {:?}", events[0].io);
    };
    assert_eq!(note.octave, 8);
}

#[test]
fn midi_locks_data_ports_even_without_bang() {
    // `E` sits in the length port of an unbanged `:`. The reference locks all
    // data ports on every frame the operator runs, so the `E` is claimed as
    // data and never moves.
    let mut orca = engine(&[":04cfE."]);
    orca.tick();
    orca.tick();
    assert_eq!(orca.grid().rows(), vec![":04cfE.".to_owned()]);
}

// ---------------------------------------------------------------------------
// `%` (mono MIDI note).
// ---------------------------------------------------------------------------

#[test]
fn mono_emits_midi_mono_variant() {
    let mut orca = engine(&[".D1...", "..%04c"]);
    let events = tick_events(&mut orca);
    assert_eq!(
        events,
        vec![OrcaEvent {
            frame: 0,
            x: 2,
            y: 1,
            io: OrcaIoEvent::MidiMono(MidiNote {
                channel: 0,
                octave: 4,
                note: 'c',
                velocity: 15,
                length: 1,
            }),
        }]
    );
}

// ---------------------------------------------------------------------------
// `!` (MIDI control change).
// ---------------------------------------------------------------------------

#[test]
fn cc_emits_value_scaled_to_127() {
    // Value scaling is `ceil(127 * raw / 35)`: `z` (35) -> 127.
    let mut orca = engine(&[".D1...", "..!05z"]);
    assert_eq!(
        tick_events(&mut orca),
        vec![OrcaEvent {
            frame: 0,
            x: 2,
            y: 1,
            io: OrcaIoEvent::MidiCc {
                channel: 0,
                knob: 5,
                value: 127,
            },
        }]
    );

    // `1` -> ceil(127 / 35) = 4.
    let mut orca = engine(&[".D1...", "..!051"]);
    let events = tick_events(&mut orca);
    assert_eq!(
        events[0].io,
        OrcaIoEvent::MidiCc {
            channel: 0,
            knob: 5,
            value: 4,
        }
    );

    // An empty value port reads as 0 (no default): scaled value 0.
    let mut orca = engine(&[".D1...", "..!05."]);
    let events = tick_events(&mut orca);
    assert_eq!(
        events[0].io,
        OrcaIoEvent::MidiCc {
            channel: 0,
            knob: 5,
            value: 0,
        }
    );
}

#[test]
fn cc_aborts_without_channel_or_knob_or_above_channel_15() {
    for rows in [
        &[".D1...", "..!.5z"], // channel empty
        &[".D1...", "..!0.z"], // knob empty
        &[".D1...", "..!g5z"], // channel 16 > 15
    ] {
        let mut orca = engine(rows);
        assert!(
            tick_events(&mut orca).is_empty(),
            "grid {rows:?} must not emit"
        );
    }
}

// ---------------------------------------------------------------------------
// `?` (MIDI pitch bend).
// ---------------------------------------------------------------------------

#[test]
fn pb_clamps_channel_and_scales_lsb_msb() {
    // Unlike `:`/`%`/`!`, the pitch-bend channel port carries a clamp to
    // 0-15 instead of an abort: `z` (35) emits on channel 15.
    // lsb `z` -> 127; msb `h` (17) -> ceil(127 * 17 / 35) = 62.
    let mut orca = engine(&[".D1...", "..?zzh"]);
    assert_eq!(
        tick_events(&mut orca),
        vec![OrcaEvent {
            frame: 0,
            x: 2,
            y: 1,
            io: OrcaIoEvent::MidiPb {
                channel: 15,
                lsb: 127,
                msb: 62,
            },
        }]
    );
}

#[test]
fn pb_aborts_without_channel_or_lsb() {
    for rows in [
        &[".D1...", "..?.zh"], // channel empty
        &[".D1...", "..?0.h"], // lsb empty
    ] {
        let mut orca = engine(rows);
        assert!(
            tick_events(&mut orca).is_empty(),
            "grid {rows:?} must not emit"
        );
    }
}

// ---------------------------------------------------------------------------
// `;` (UDP).
// ---------------------------------------------------------------------------

#[test]
fn udp_concatenates_message_until_empty_cell() {
    let mut orca = engine(&[".D1...", "..;hi."]);
    assert_eq!(
        tick_events(&mut orca),
        vec![OrcaEvent {
            frame: 0,
            x: 2,
            y: 1,
            io: OrcaIoEvent::Udp("hi".to_owned()),
        }]
    );
}

#[test]
fn udp_emits_empty_message_when_banged() {
    // Faithful to the reference: `;` has no empty-message guard (unlike `$`).
    let mut orca = engine(&[".D1...", "..;..."]);
    assert_eq!(
        tick_events(&mut orca),
        vec![OrcaEvent {
            frame: 0,
            x: 2,
            y: 1,
            io: OrcaIoEvent::Udp(String::new()),
        }]
    );
}

#[test]
fn udp_locks_message_cells_even_without_bang() {
    // The eastward message scan locks every cell it visits, so the `E` is
    // frozen as message data even though the `;` never fires.
    let mut orca = engine(&[";E.."]);
    orca.tick();
    orca.tick();
    assert_eq!(orca.grid().rows(), vec![";E..".to_owned()]);
    assert!(orca.events().is_empty());
}

// ---------------------------------------------------------------------------
// `=` (OSC).
// ---------------------------------------------------------------------------

#[test]
fn osc_emits_path_glyph_and_args_until_empty_cell() {
    let mut orca = engine(&[".D1....", "..=a12."]);
    assert_eq!(
        tick_events(&mut orca),
        vec![OrcaEvent {
            frame: 0,
            x: 2,
            y: 1,
            io: OrcaIoEvent::Osc {
                path: 'a',
                args: "12".to_owned(),
            },
        }]
    );
}

#[test]
fn osc_aborts_without_path() {
    let mut orca = engine(&[".D1...", "..=.12"]);
    assert!(tick_events(&mut orca).is_empty());
}

// ---------------------------------------------------------------------------
// `$` (self / command).
// ---------------------------------------------------------------------------

#[test]
fn command_emits_message_string_when_banged() {
    let mut orca = engine(&[".D1.....", "..$bpm90"]);
    assert_eq!(
        tick_events(&mut orca),
        vec![OrcaEvent {
            frame: 0,
            x: 2,
            y: 1,
            io: OrcaIoEvent::Command("bpm90".to_owned()),
        }]
    );
}

#[test]
fn command_requires_nonempty_message() {
    // `$` (unlike `;`) returns without emitting when the message is empty.
    let mut orca = engine(&[".D1...", "..$..."]);
    assert!(tick_events(&mut orca).is_empty());
}

#[test]
fn command_locks_message_cells_even_without_bang() {
    let mut orca = engine(&["$E.."]);
    orca.tick();
    orca.tick();
    assert_eq!(orca.grid().rows(), vec!["$E..".to_owned()]);
    assert!(orca.events().is_empty());
}

// ---------------------------------------------------------------------------
// Note transposition (reference `transpose.js` + `io/midi.js`).
// ---------------------------------------------------------------------------

#[test]
fn midi_note_id_matches_reference_transpose_table() {
    // Uppercase letters are naturals, lowercase are sharps.
    assert_eq!(midi_note_id('C', 3), Some(60), "middle C");
    assert_eq!(midi_note_id('c', 3), Some(61), "c is C#");
    assert_eq!(midi_note_id('D', 3), Some(62));
    assert_eq!(midi_note_id('A', 3), Some(69));
    assert_eq!(midi_note_id('B', 3), Some(71));
    // Letters past G wrap upward through the octaves.
    assert_eq!(midi_note_id('H', 3), Some(69), "H aliases A");
    assert_eq!(midi_note_id('J', 3), Some(72), "J is C one octave up");
    assert_eq!(midi_note_id('Q', 3), Some(84), "Q is C two octaves up");
    // Catch entries: sharps that do not exist resolve upward (E# = F,
    // B# = C).
    assert_eq!(midi_note_id('e', 3), Some(65), "e catches to F");
    assert_eq!(midi_note_id('b', 3), Some(72), "b catches to C4");
}

#[test]
fn midi_note_id_clamps_octave_and_note_number() {
    // Octave offset clamps to 8: `Z` (E + 3 octaves) at octave 8 stays at
    // octave 8 -> 8 * 12 + 4 + 24 = 124.
    assert_eq!(midi_note_id('Z', 8), Some(124));
    // The note number clamps to 127: `B` at octave 8 would be 131.
    assert_eq!(midi_note_id('B', 8), Some(127));
}

#[test]
fn midi_note_id_rejects_non_note_glyphs() {
    assert_eq!(midi_note_id('5', 3), None);
    assert_eq!(midi_note_id('.', 3), None);
    assert_eq!(midi_note_id('*', 3), None);
}

// ---------------------------------------------------------------------------
// Audio bridge: MIDI-note events flow into the existing publish seam.
// ---------------------------------------------------------------------------

#[test]
fn midi_events_flow_to_sample_events_relative_to_middle_c() {
    let note = MidiNote {
        channel: 0,
        octave: 4,
        note: 'C',
        velocity: 15,
        length: 1,
    };
    let event = sample_event_from_orca(&midi_event(OrcaIoEvent::Midi(note)), 0, 16, "tri")
        .expect("valid frame span")
        .expect("note events map to sample events");
    // Octave 4 C is MIDI 72: +12 semitones above middle C doubles the rate.
    assert!((event.value.rate() - 2.0).abs() < 1e-12);

    let low = MidiNote { octave: 2, ..note };
    let event = sample_event_from_orca(&midi_event(OrcaIoEvent::Midi(low)), 0, 16, "tri")
        .expect("valid frame span")
        .expect("note events map to sample events");
    // Octave 2 C is MIDI 48: one octave below middle C halves the rate.
    assert!((event.value.rate() - 0.5).abs() < 1e-12);
}

#[test]
fn mono_events_reach_audio_and_non_note_io_does_not() {
    let note = MidiNote {
        channel: 3,
        octave: 3,
        note: 'C',
        velocity: 15,
        length: 1,
    };
    let event = sample_event_from_orca(&midi_event(OrcaIoEvent::MidiMono(note)), 0, 16, "tri")
        .expect("valid frame span")
        .expect("mono notes map to sample events");
    assert!(
        (event.value.rate() - 1.0).abs() < 1e-12,
        "middle C plays the sample at its base pitch"
    );

    for io in [
        OrcaIoEvent::Udp("hi".to_owned()),
        OrcaIoEvent::Osc {
            path: 'a',
            args: "12".to_owned(),
        },
        OrcaIoEvent::Command("bpm90".to_owned()),
        OrcaIoEvent::MidiCc {
            channel: 0,
            knob: 5,
            value: 127,
        },
        OrcaIoEvent::MidiPb {
            channel: 0,
            lsb: 0,
            msb: 64,
        },
    ] {
        assert_eq!(
            sample_event_from_orca(&midi_event(io.clone()), 0, 16, "tri")
                .expect("valid frame span"),
            None,
            "{io:?} must not produce a sample event"
        );
    }
}

#[test]
fn untransposable_note_glyph_produces_no_sample_event() {
    // The reference pushes such notes onto the MIDI stack and silently drops
    // them at send time (`transpose` returns null); the audio bridge drops
    // them at conversion time.
    let note = MidiNote {
        channel: 0,
        octave: 4,
        note: '*',
        velocity: 15,
        length: 1,
    };
    assert_eq!(
        sample_event_from_orca(&midi_event(OrcaIoEvent::Midi(note)), 0, 16, "tri")
            .expect("valid frame span"),
        None
    );
}

#[test]
fn materialize_cycle_carries_note_events_and_skips_other_io() {
    // One frame: `:` emits a note and `;` emits a UDP message. Only the note
    // reaches the sample-event batch; the UDP event stays on the engine's
    // per-tick event list for future transports.
    let mut orca = engine(&[".D1....D1...", "..:04c..;hi."]);
    let events = materialize_cycle(&mut orca, 1, "tri").expect("materializes");
    assert_eq!(events.len(), 1, "only the MIDI note reaches audio");
    let expected_rate = (13.0 / 12.0_f64).exp2();
    assert!((events[0].value.rate() - expected_rate).abs() < 1e-12);
    assert_eq!(
        orca.events().len(),
        2,
        "both IO events are collected per tick"
    );
}

// ---------------------------------------------------------------------------
// Grid alphabet.
// ---------------------------------------------------------------------------

#[test]
fn io_glyphs_are_valid_on_the_grid() {
    for glyph in [':', '%', '!', '?', ';', '=', '$'] {
        assert!(is_valid_glyph(glyph), "{glyph:?} must be a valid glyph");
    }
    assert!(!is_valid_glyph('#'), "comments remain out of scope");
}
