//! Behavior tests for Orca v6: real IO transports for the grid surface.
//!
//! The v3 typed [`OrcaIoEvent`]s now reach real destinations: `;` sends the
//! raw message string as a UDP datagram (reference default port 49161), `=`
//! sends an OSC message with address `/<path>` and one int32 per base-36
//! arg glyph (default 49162), and the MIDI family (`:` `%` `!` `?`)
//! assembles the exact byte streams of the reference io layer
//! (`io/midi.js`, `io/cc.js`, `io/mono.js`). Everything runs hardware-free:
//! UDP/OSC against loopback sockets bound by the test, MIDI against a
//! recording mock behind the `MidiSink` trait, and all timing against a
//! synchronous dispatcher driven by explicit instants. See
//! `docs/design/orca-surface.md` section 12 and ADR 0010.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use orpheus_lang::orca::{
    MidiNote, OrcaEngine, OrcaIoEvent, OrcaPublisher, materialize_cycle_io, midi_note_id,
};
use orpheus_lang::orca::{
    RecordingMidiSink, ScheduledIoEvent, TransportConfig, TransportDispatcher, TransportHandle,
    cycle_schedule,
};

const FRAME: Duration = Duration::from_millis(125);

fn loopback_receiver() -> (UdpSocket, SocketAddr) {
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind loopback receiver");
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    let address = socket.local_addr().expect("local addr");
    (socket, address)
}

fn receive(socket: &UdpSocket) -> Vec<u8> {
    let mut buffer = [0_u8; 1024];
    let (length, _) = socket.recv_from(&mut buffer).expect("datagram arrives");
    buffer[..length].to_vec()
}

/// A dispatcher with a recording MIDI sink and both sockets aimed at
/// throwaway loopback receivers (so no test traffic escapes the host).
fn midi_dispatcher() -> (TransportDispatcher, RecordingMidiSink) {
    let (_, udp_target) = loopback_receiver();
    let (_, osc_target) = loopback_receiver();
    let mut dispatcher = TransportDispatcher::new(&TransportConfig {
        udp_target,
        osc_target,
        ..TransportConfig::default()
    });
    let recording = RecordingMidiSink::new();
    dispatcher.set_midi(Some(Box::new(recording.clone())));
    (dispatcher, recording)
}

const fn note(channel: u8, glyph: char, octave: u8, velocity: u8, length: u8) -> MidiNote {
    MidiNote {
        channel,
        octave,
        note: glyph,
        velocity,
        length,
    }
}

fn schedule(
    dispatcher: &mut TransportDispatcher,
    io: OrcaIoEvent,
    fire_at: Instant,
) -> Vec<String> {
    dispatcher.schedule(ScheduledIoEvent {
        fire_at,
        frame_duration: FRAME,
        io,
    });
    dispatcher.run_due(fire_at)
}

// ---------------------------------------------------------------------------
// MIDI notes: on/off pairs, timing, retrigger, mono cut.
// ---------------------------------------------------------------------------

#[test]
fn poly_note_presses_then_releases_after_its_length_in_frames() {
    let (mut dispatcher, recording) = midi_dispatcher();
    let start = Instant::now();
    // `:13C` with velocity f (15) and length 2: channel 1, middle C (60).
    let errors = schedule(
        &mut dispatcher,
        OrcaIoEvent::Midi(note(1, 'C', 3, 15, 2)),
        start,
    );
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    assert_eq!(
        recording.messages(),
        vec![vec![0x91, 60, 119]],
        "only the note-on fires at the deadline",
    );

    // Half a frame shy of the note length: still sounding.
    dispatcher.run_due(start + FRAME + FRAME / 2);
    assert_eq!(recording.messages().len(), 1);

    // At exactly length frames the note-off fires with the same velocity
    // byte (the reference releases with the item's velocity).
    dispatcher.run_due(start + 2 * FRAME);
    assert_eq!(
        recording.messages(),
        vec![vec![0x91, 60, 119], vec![0x81, 60, 119]],
    );
    assert!(!dispatcher.has_pending());
}

#[test]
fn zero_length_note_presses_and_releases_in_the_same_pass() {
    // The reference presses and immediately releases a length-0 note in
    // one `run()` pass.
    let (mut dispatcher, recording) = midi_dispatcher();
    let start = Instant::now();
    schedule(
        &mut dispatcher,
        OrcaIoEvent::Midi(note(0, 'C', 3, 16, 0)),
        start,
    );
    assert_eq!(
        recording.messages(),
        vec![vec![0x90, 60, 127], vec![0x80, 60, 127]],
    );
}

#[test]
fn duplicate_poly_note_releases_before_repressing_and_supersedes_the_old_off() {
    let (mut dispatcher, recording) = midi_dispatcher();
    let start = Instant::now();
    // Two identical notes (channel, octave, glyph) one frame apart, each
    // 4 frames long (`midi.js` `push()` releases duplicates on push).
    schedule(
        &mut dispatcher,
        OrcaIoEvent::Midi(note(0, 'C', 3, 15, 4)),
        start,
    );
    schedule(
        &mut dispatcher,
        OrcaIoEvent::Midi(note(0, 'C', 3, 15, 4)),
        start + FRAME,
    );
    assert_eq!(
        recording.messages(),
        vec![
            vec![0x90, 60, 119], // first press
            vec![0x80, 60, 119], // duplicate cut
            vec![0x90, 60, 119], // second press
        ],
    );

    // The first note's pending off (due at start + 4 frames) is stale and
    // must not double-release the retriggered note.
    dispatcher.run_due(start + 4 * FRAME);
    assert_eq!(recording.messages().len(), 3);

    // The retriggered note releases at its own deadline.
    dispatcher.run_due(start + 5 * FRAME);
    assert_eq!(recording.messages().len(), 4);
    assert_eq!(recording.messages()[3], vec![0x80, 60, 119]);
}

#[test]
fn mono_note_cuts_the_previous_note_on_the_same_channel() {
    let (mut dispatcher, recording) = midi_dispatcher();
    let start = Instant::now();
    // `%` holds one note per channel (`mono.js` keys its stack by channel):
    // a new note first releases the old one, with the OLD note's bytes.
    schedule(
        &mut dispatcher,
        OrcaIoEvent::MidiMono(note(2, 'C', 3, 15, 8)),
        start,
    );
    schedule(
        &mut dispatcher,
        OrcaIoEvent::MidiMono(note(2, 'D', 3, 15, 8)),
        start + FRAME,
    );
    let d_id = midi_note_id('D', 3).expect("D transposes");
    assert_eq!(
        recording.messages(),
        vec![
            vec![0x92, 60, 119],   // C on
            vec![0x82, 60, 119],   // C cut by the new note
            vec![0x92, d_id, 119]  // D on
        ],
    );
    // The cut note's pending off is stale; D releases on schedule.
    dispatcher.run_due(start + 9 * FRAME);
    assert_eq!(recording.messages().len(), 4);
    assert_eq!(recording.messages()[3], vec![0x82, d_id, 119]);
}

#[test]
fn mono_notes_on_different_channels_do_not_cut_each_other() {
    let (mut dispatcher, recording) = midi_dispatcher();
    let start = Instant::now();
    schedule(
        &mut dispatcher,
        OrcaIoEvent::MidiMono(note(0, 'C', 3, 15, 8)),
        start,
    );
    schedule(
        &mut dispatcher,
        OrcaIoEvent::MidiMono(note(1, 'D', 3, 15, 8)),
        start + FRAME,
    );
    let messages = recording.messages();
    assert_eq!(messages.len(), 2, "two note-ons, no cut: {messages:?}");
    assert_eq!(messages[0][0], 0x90);
    assert_eq!(messages[1][0], 0x91);
}

#[test]
fn untransposable_note_glyphs_are_dropped_at_send_time() {
    // The reference's `trigger()` returns without sending when the glyph
    // is outside the transpose table.
    let (mut dispatcher, recording) = midi_dispatcher();
    let errors = schedule(
        &mut dispatcher,
        OrcaIoEvent::Midi(note(0, '5', 3, 15, 1)),
        Instant::now(),
    );
    assert!(errors.is_empty());
    assert!(recording.messages().is_empty());
    assert!(!dispatcher.has_pending(), "no orphan note-off is queued");
}

// ---------------------------------------------------------------------------
// MIDI CC and pitch bend bytes.
// ---------------------------------------------------------------------------

#[test]
fn cc_event_sends_the_reference_bytes_with_knob_offset() {
    let (mut dispatcher, recording) = midi_dispatcher();
    schedule(
        &mut dispatcher,
        OrcaIoEvent::MidiCc {
            channel: 2,
            knob: 3,
            value: 127,
        },
        Instant::now(),
    );
    // `cc.js`: [0xB0 + channel, offset(64) + knob, value].
    assert_eq!(recording.messages(), vec![vec![0xB2, 67, 127]]);
}

#[test]
fn pb_event_sends_raw_lsb_msb_bytes() {
    let (mut dispatcher, recording) = midi_dispatcher();
    schedule(
        &mut dispatcher,
        OrcaIoEvent::MidiPb {
            channel: 5,
            lsb: 11,
            msb: 96,
        },
        Instant::now(),
    );
    // `cc.js`: [0xE0 + channel, lsb, msb].
    assert_eq!(recording.messages(), vec![vec![0xE5, 11, 96]]);
}

// ---------------------------------------------------------------------------
// Dispatcher routing and non-MIDI families.
// ---------------------------------------------------------------------------

#[test]
fn dispatcher_routes_each_event_family_to_its_transport() {
    let (udp_receiver, udp_target) = loopback_receiver();
    let (osc_receiver, osc_target) = loopback_receiver();
    let mut dispatcher = TransportDispatcher::new(&TransportConfig {
        udp_target,
        osc_target,
        ..TransportConfig::default()
    });
    let recording = RecordingMidiSink::new();
    dispatcher.set_midi(Some(Box::new(recording.clone())));

    let now = Instant::now();
    for io in [
        OrcaIoEvent::Udp("hello5".to_owned()),
        OrcaIoEvent::Osc {
            path: 'a',
            args: "0cz".to_owned(),
        },
        OrcaIoEvent::Midi(note(0, 'C', 3, 15, 1)),
        OrcaIoEvent::Command("bpm140".to_owned()),
    ] {
        let errors = schedule(&mut dispatcher, io, now);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    }

    // UDP: the raw message string, verbatim.
    assert_eq!(receive(&udp_receiver), b"hello5");

    // OSC: `/a` with the base-36 int values of `0cz`.
    let payload = receive(&osc_receiver);
    let (_, packet) = rosc::decoder::decode_udp(&payload).expect("valid OSC packet");
    let rosc::OscPacket::Message(message) = packet else {
        panic!("expected a message packet");
    };
    assert_eq!(message.addr, "/a");
    assert_eq!(
        message.args,
        vec![
            rosc::OscType::Int(0),
            rosc::OscType::Int(12),
            rosc::OscType::Int(35)
        ],
    );

    // MIDI: exactly one note-on (`$` commands are dropped uninterpreted).
    assert_eq!(recording.messages(), vec![vec![0x90, 60, 119]]);
}

#[test]
fn events_do_not_fire_before_their_deadline() {
    let (udp_receiver, udp_target) = loopback_receiver();
    let (_, osc_target) = loopback_receiver();
    let mut dispatcher = TransportDispatcher::new(&TransportConfig {
        udp_target,
        osc_target,
        ..TransportConfig::default()
    });
    let now = Instant::now();
    dispatcher.schedule(ScheduledIoEvent {
        fire_at: now + FRAME,
        frame_duration: FRAME,
        io: OrcaIoEvent::Udp("late".to_owned()),
    });
    dispatcher.run_due(now);
    udp_receiver
        .set_read_timeout(Some(Duration::from_millis(50)))
        .expect("set read timeout");
    let mut buffer = [0_u8; 16];
    assert!(
        udp_receiver.recv_from(&mut buffer).is_err(),
        "nothing may arrive before the deadline",
    );
    assert_eq!(dispatcher.next_due(), Some(now + FRAME));
    dispatcher.run_due(now + FRAME);
    udp_receiver
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    assert_eq!(receive(&udp_receiver), b"late");
}

#[test]
fn same_deadline_events_fire_in_submission_order() {
    // Same-frame events must keep the grid's scan order on the wire.
    let (udp_receiver, udp_target) = loopback_receiver();
    let (_, osc_target) = loopback_receiver();
    let mut dispatcher = TransportDispatcher::new(&TransportConfig {
        udp_target,
        osc_target,
        ..TransportConfig::default()
    });
    let now = Instant::now();
    for message in ["first", "second", "third"] {
        dispatcher.schedule(ScheduledIoEvent {
            fire_at: now,
            frame_duration: FRAME,
            io: OrcaIoEvent::Udp(message.to_owned()),
        });
    }
    dispatcher.run_due(now);
    assert_eq!(receive(&udp_receiver), b"first");
    assert_eq!(receive(&udp_receiver), b"second");
    assert_eq!(receive(&udp_receiver), b"third");
}

#[test]
fn notes_without_a_midi_sink_are_dropped_silently() {
    let (_, udp_target) = loopback_receiver();
    let (_, osc_target) = loopback_receiver();
    let mut dispatcher = TransportDispatcher::new(&TransportConfig {
        udp_target,
        osc_target,
        ..TransportConfig::default()
    });
    let errors = schedule(
        &mut dispatcher,
        OrcaIoEvent::Midi(note(0, 'C', 3, 15, 1)),
        Instant::now(),
    );
    assert!(errors.is_empty(), "no sink means a silent no-op");
}

#[test]
fn flush_note_offs_releases_every_sounding_note() {
    let (mut dispatcher, recording) = midi_dispatcher();
    let start = Instant::now();
    schedule(
        &mut dispatcher,
        OrcaIoEvent::Midi(note(0, 'C', 3, 15, 32)),
        start,
    );
    schedule(
        &mut dispatcher,
        OrcaIoEvent::MidiMono(note(1, 'D', 3, 15, 32)),
        start,
    );
    assert_eq!(recording.messages().len(), 2, "both notes sounding");
    let errors = dispatcher.flush_note_offs();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    let messages = recording.messages();
    assert_eq!(messages.len(), 4, "both notes released: {messages:?}");
    assert!(messages[2][0] & 0xF0 == 0x80 && messages[3][0] & 0xF0 == 0x80);
    assert!(!dispatcher.has_pending());
}

// ---------------------------------------------------------------------------
// The worker thread.
// ---------------------------------------------------------------------------

#[test]
fn worker_thread_delivers_scheduled_events() {
    let (udp_receiver, udp_target) = loopback_receiver();
    let (_, osc_target) = loopback_receiver();
    let mut handle = TransportHandle::spawn_with_config(TransportConfig {
        udp_target,
        osc_target,
        ..TransportConfig::default()
    });
    let recording = RecordingMidiSink::new();
    handle.set_midi_sink(Box::new(recording.clone()), "test-sink");
    assert_eq!(handle.midi_port(), Some("test-sink"));

    let now = Instant::now();
    handle.schedule(vec![
        ScheduledIoEvent {
            fire_at: now,
            frame_duration: FRAME,
            io: OrcaIoEvent::Udp("from-worker".to_owned()),
        },
        ScheduledIoEvent {
            fire_at: now,
            frame_duration: FRAME,
            io: OrcaIoEvent::Midi(note(0, 'C', 3, 15, 1)),
        },
    ]);

    assert_eq!(receive(&udp_receiver), b"from-worker");
    let deadline = Instant::now() + Duration::from_secs(5);
    while recording.messages().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(recording.messages()[0], vec![0x90, 60, 119]);
    assert!(handle.poll_status().is_empty(), "no transport errors");
}

#[test]
fn dropping_the_handle_releases_hanging_notes() {
    let (_, udp_target) = loopback_receiver();
    let (_, osc_target) = loopback_receiver();
    let mut handle = TransportHandle::spawn_with_config(TransportConfig {
        udp_target,
        osc_target,
        ..TransportConfig::default()
    });
    let recording = RecordingMidiSink::new();
    handle.set_midi_sink(Box::new(recording.clone()), "test-sink");
    handle.schedule(vec![ScheduledIoEvent {
        fire_at: Instant::now(),
        frame_duration: FRAME,
        io: OrcaIoEvent::Midi(note(0, 'C', 3, 15, 32)),
    }]);
    let deadline = Instant::now() + Duration::from_secs(5);
    while recording.messages().is_empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(recording.messages(), vec![vec![0x90, 60, 119]]);
    // Dropping joins the worker, which flushes the pending note-off long
    // before its 32-frame deadline.
    drop(handle);
    assert_eq!(
        recording.messages(),
        vec![vec![0x90, 60, 119], vec![0x80, 60, 0]],
        "shutdown releases the sounding note",
    );
}

// ---------------------------------------------------------------------------
// Publisher seam: IO events come out of materialization frame-stamped.
// ---------------------------------------------------------------------------

#[test]
fn materialize_cycle_io_stamps_events_with_their_cycle_frame() {
    // `D1` bangs every frame; `;ab` east of the bang cell sends "ab".
    let mut engine = OrcaEngine::from_rows(&[".D1...", ".*;ab."]).expect("valid grid");
    let cycle = materialize_cycle_io(&mut engine, 4, "tri").expect("materializes");
    assert!(cycle.audio.is_empty(), "UDP events never reach audio");
    assert_eq!(cycle.io.len(), 4, "one send per frame: {:?}", cycle.io);
    for (index, stamped) in cycle.io.iter().enumerate() {
        assert_eq!(stamped.frame_in_cycle, index as u64);
        assert_eq!(stamped.event.io, OrcaIoEvent::Udp("ab".to_owned()));
    }
}

#[test]
fn publisher_poll_io_returns_both_audio_and_io_events() {
    // Row 0: `D1` bangs every frame. Row 1: the bang lands at (1,1), west
    // of `:` -> a note per frame; row 2 is free for the note's output.
    let engine = OrcaEngine::from_rows(&[".D1....", "..:04c.", "......."]).expect("valid grid");
    let mut publisher = OrcaPublisher::new(engine, 4, "tri");
    publisher.start();
    let cycle = publisher
        .poll_io(0)
        .expect("poll succeeds")
        .expect("first poll materializes");
    assert_eq!(cycle.audio.len(), 4, "notes reach the audio batch");
    assert_eq!(cycle.io.len(), 4, "and the transport list");
    assert!(
        cycle
            .io
            .iter()
            .all(|stamped| matches!(stamped.event.io, OrcaIoEvent::Midi(_))),
    );
    assert!(
        publisher.poll_io(0).expect("poll succeeds").is_none(),
        "unchanged boundary polls nothing",
    );
}

// ---------------------------------------------------------------------------
// Wall-clock scheduling seam.
// ---------------------------------------------------------------------------

#[test]
fn cycle_schedule_maps_grid_frames_onto_the_next_engine_cycle() {
    let now = Instant::now();
    // 120 BPM -> 2 s cycles; engine 3/4 through the current cycle -> next
    // boundary in 0.5 s; 16 grid frames -> 125 ms frames.
    let (start, frame) = cycle_schedule(now, 120.0, 72_000, 0, 96_000, 16, true);
    assert_eq!(start - now, Duration::from_millis(500));
    assert_eq!(frame, Duration::from_millis(125));
    // Frame 4 of the scheduled cycle lands half a second past the boundary.
    assert_eq!((start + frame * 4) - now, Duration::from_millis(1000));
}
