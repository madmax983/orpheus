//! Behavior tests for Orca v7: the `$` command interpreter, MIDI clock
//! out, and UDP command input.
//!
//! - **Commands** follow the reference `commander.js` grammar (name before
//!   the first `:`, cleaned and lowercased; two-letter shorthands;
//!   `parseInt` values) and map onto Orpheus where an equivalent exists:
//!   `bpm`/`apm` → global tempo (clamped to the reference's 60-300),
//!   `frame`/`rewind`/`skip` → the grid frame counter (clamped to
//!   0-9999999), `play`/`stop` → the grid clock. The dispatcher collects
//!   command strings at their wall-clock deadlines for the host.
//! - **MIDI clock** transcribes `io/midi.js` `sendClock*`: 0xFA on start,
//!   six 0xF8 ticks per 16th-note grid frame (24 PPQN), 0xFC on stop.
//! - **UDP input** mirrors `udp.js` `selectInput` → `commander.trigger`:
//!   received datagrams speak the same command language.
//!
//! Everything is hardware-free: MIDI against the recording mock, timing
//! against the synchronous dispatcher, UDP against loopback sockets. See
//! `docs/design/orca-surface.md` section 13 and ADR 0011.

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use orpheus_lang::orca::transport::{
    RecordingMidiSink, ScheduledIoEvent, TransportConfig, TransportDispatcher, TransportHandle,
    UdpCommandListener,
};
use orpheus_lang::orca::{CommandOutcome, OrcaCommand, OrcaEngine, OrcaIoEvent, parse_command};

/// One grid frame at 120 BPM (16th note), divisible by 6 so clock tick
/// deadlines land on exact milliseconds.
const FRAME: Duration = Duration::from_millis(120);
const TICK: Duration = Duration::from_millis(20);

fn loopback_receiver() -> (UdpSocket, SocketAddr) {
    let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind loopback receiver");
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    let address = socket.local_addr().expect("local addr");
    (socket, address)
}

/// A dispatcher with a recording MIDI sink and both sockets aimed at
/// throwaway loopback receivers.
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

fn count_bytes(recording: &RecordingMidiSink, byte: u8) -> usize {
    recording
        .messages()
        .iter()
        .filter(|message| message.as_slice() == [byte])
        .count()
}

// ---------------------------------------------------------------------------
// Command grammar spot checks (the full grammar suite lives with the
// parser; these pin the public API surface).
// ---------------------------------------------------------------------------

#[test]
fn command_parser_maps_the_supported_reference_commands() {
    assert_eq!(
        parse_command("bpm:140"),
        CommandOutcome::Apply(OrcaCommand::Bpm(140))
    );
    assert_eq!(
        parse_command("fr:8"),
        CommandOutcome::Apply(OrcaCommand::Frame(8))
    );
    assert_eq!(
        parse_command("play"),
        CommandOutcome::Apply(OrcaCommand::Play)
    );
    assert_eq!(
        parse_command("stop"),
        CommandOutcome::Apply(OrcaCommand::Stop)
    );
    assert_eq!(
        parse_command("color:aaa"),
        CommandOutcome::Divergent("color")
    );
    assert_eq!(
        parse_command("xyzzy"),
        CommandOutcome::Unknown("xyzzy".to_owned())
    );
}

// ---------------------------------------------------------------------------
// The grid engine's frame counter is host-settable (frame/rewind/skip).
// ---------------------------------------------------------------------------

#[test]
fn set_frame_re_anchors_frame_phased_operators() {
    // `1C4` counts floor(frame / 1) mod 4 below itself each frame.
    let mut engine = OrcaEngine::from_rows(&["1C4...", "......"]).expect("valid grid");
    engine.tick();
    assert_eq!(engine.grid().glyph_at(1, 1), Some('0'), "frame 0");
    engine.set_frame(7);
    assert_eq!(engine.frame(), 7);
    engine.tick();
    assert_eq!(
        engine.grid().glyph_at(1, 1),
        Some('3'),
        "the clock re-anchors to 7 mod 4",
    );
    assert_eq!(engine.frame(), 8, "ticking advances from the new counter");
}

// ---------------------------------------------------------------------------
// Dispatcher: `$` commands are collected at their deadlines, not dropped.
// ---------------------------------------------------------------------------

#[test]
fn dispatcher_collects_command_events_at_their_deadlines() {
    let (mut dispatcher, recording) = midi_dispatcher();
    let start = Instant::now();
    dispatcher.schedule(ScheduledIoEvent {
        fire_at: start + FRAME,
        frame_duration: FRAME,
        io: OrcaIoEvent::Command("bpm:140".to_owned()),
    });
    dispatcher.schedule(ScheduledIoEvent {
        fire_at: start,
        frame_duration: FRAME,
        io: OrcaIoEvent::Command("play".to_owned()),
    });

    let errors = dispatcher.run_due(start);
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    assert_eq!(
        dispatcher.take_commands(),
        vec!["play".to_owned()],
        "only the due command fires",
    );

    dispatcher.run_due(start + FRAME);
    assert_eq!(dispatcher.take_commands(), vec!["bpm:140".to_owned()]);
    assert!(
        dispatcher.take_commands().is_empty(),
        "draining is destructive"
    );
    assert!(
        recording.messages().is_empty(),
        "commands never reach the MIDI sink",
    );
}

// ---------------------------------------------------------------------------
// MIDI clock out: 0xFA / six 0xF8 per frame / 0xFC.
// ---------------------------------------------------------------------------

#[test]
fn clock_start_sends_start_byte_then_six_ticks_per_frame() {
    let (mut dispatcher, recording) = midi_dispatcher();
    let start = Instant::now();
    let errors = dispatcher.clock_start(start, FRAME);
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    assert!(dispatcher.clock_running());

    dispatcher.run_due(start);
    assert_eq!(
        recording.messages(),
        vec![vec![0xFA], vec![0xF8]],
        "start byte, then the first tick at the anchor",
    );

    // One tick every FRAME/6: at just shy of one frame (119 ms of the
    // 120 ms FRAME), exactly six ticks.
    dispatcher.run_due(start + Duration::from_millis(119));
    assert_eq!(
        count_bytes(&recording, 0xF8),
        6,
        "6 ticks per 16th = 24 PPQN"
    );

    // The seventh tick opens the next frame, anchored drift-free at
    // exactly start + FRAME.
    dispatcher.run_due(start + FRAME);
    assert_eq!(count_bytes(&recording, 0xF8), 7);

    // Ten frames on (just shy of 10 * FRAME = 1200 ms), still exactly
    // 6 ticks per frame — no cumulative drift from the tick-to-tick
    // rescheduling.
    dispatcher.run_due(start + Duration::from_millis(1199));
    assert_eq!(count_bytes(&recording, 0xF8), 60);
}

#[test]
fn clock_stop_sends_stop_byte_and_ceases_ticks() {
    let (mut dispatcher, recording) = midi_dispatcher();
    let start = Instant::now();
    dispatcher.clock_start(start, FRAME);
    dispatcher.run_due(start + TICK);
    assert_eq!(count_bytes(&recording, 0xF8), 2);

    let errors = dispatcher.clock_stop();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    assert!(!dispatcher.clock_running());
    assert_eq!(count_bytes(&recording, 0xFC), 1, "stop byte sent");

    dispatcher.run_due(start + 10 * FRAME);
    assert_eq!(count_bytes(&recording, 0xF8), 2, "no ticks after stop");
    assert!(!dispatcher.has_pending(), "stale tick actions drained");

    // Stopping a stopped clock is a no-op (no second 0xFC).
    dispatcher.clock_stop();
    assert_eq!(count_bytes(&recording, 0xFC), 1);
}

#[test]
fn clock_period_update_retunes_ticks_from_the_next_tick() {
    let (mut dispatcher, recording) = midi_dispatcher();
    let start = Instant::now();
    dispatcher.clock_start(start, FRAME);
    dispatcher.run_due(start);
    assert_eq!(count_bytes(&recording, 0xF8), 1);

    // Halving the frame duration (doubling the tempo) halves the tick
    // period from the next tick on: the second tick was already anchored
    // at start + TICK, later ticks come every TICK/2.
    dispatcher.set_clock_frame_duration(FRAME / 2);
    dispatcher.run_due(start + TICK);
    assert_eq!(count_bytes(&recording, 0xF8), 2);
    dispatcher.run_due(start + TICK + TICK / 2);
    assert_eq!(count_bytes(&recording, 0xF8), 3, "retimed tick spacing");
}

#[test]
fn clock_interleaves_with_note_events_without_disturbing_them() {
    use orpheus_lang::orca::MidiNote;

    let (mut dispatcher, recording) = midi_dispatcher();
    let start = Instant::now();
    dispatcher.clock_start(start, FRAME);
    dispatcher.schedule(ScheduledIoEvent {
        fire_at: start,
        frame_duration: FRAME,
        io: OrcaIoEvent::Midi(MidiNote {
            channel: 0,
            octave: 3,
            note: 'C',
            velocity: 15,
            length: 1,
        }),
    });
    dispatcher.run_due(start + FRAME);
    let messages = recording.messages();
    assert!(messages.contains(&vec![0x90, 60, 119]), "note-on sent");
    assert!(messages.contains(&vec![0x80, 60, 119]), "note-off sent");
    assert_eq!(count_bytes(&recording, 0xF8), 7, "clock unaffected");
}

#[test]
fn flush_stops_a_running_clock_with_the_stop_byte() {
    let (mut dispatcher, recording) = midi_dispatcher();
    dispatcher.clock_start(Instant::now(), FRAME);
    let errors = dispatcher.flush_note_offs();
    assert!(errors.is_empty(), "unexpected errors: {errors:?}");
    assert!(!dispatcher.clock_running());
    assert_eq!(count_bytes(&recording, 0xFC), 1, "shutdown sends 0xFC");
}

// ---------------------------------------------------------------------------
// Worker round trips: commands come back to the host; clock runs on the
// IO thread.
// ---------------------------------------------------------------------------

#[test]
fn worker_returns_fired_commands_to_the_host() {
    let handle = TransportHandle::spawn();
    handle.schedule(vec![ScheduledIoEvent {
        fire_at: Instant::now(),
        frame_duration: FRAME,
        io: OrcaIoEvent::Command("frame:3".to_owned()),
    }]);
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut commands = Vec::new();
    while commands.is_empty() && Instant::now() < deadline {
        commands = handle.poll_commands();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(commands, vec!["frame:3".to_owned()]);
}

#[test]
fn worker_runs_the_midi_clock_through_the_sink() {
    let mut handle = TransportHandle::spawn();
    let recording = RecordingMidiSink::new();
    handle.set_midi_sink(Box::new(recording.clone()), "test-sink");
    handle.start_clock(Duration::from_millis(30));

    let deadline = Instant::now() + Duration::from_secs(5);
    while count_bytes(&recording, 0xF8) < 3 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(count_bytes(&recording, 0xFA), 1, "start byte first");
    assert!(count_bytes(&recording, 0xF8) >= 3, "ticks flowing");

    handle.stop_clock();
    let deadline = Instant::now() + Duration::from_secs(5);
    while count_bytes(&recording, 0xFC) == 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(count_bytes(&recording, 0xFC), 1, "stop byte sent");
    let settled = count_bytes(&recording, 0xF8);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(count_bytes(&recording, 0xF8), settled, "ticks ceased");
}

// ---------------------------------------------------------------------------
// UDP command input.
// ---------------------------------------------------------------------------

#[test]
fn udp_listener_delivers_command_strings_in_arrival_order() {
    let listener =
        UdpCommandListener::bind((Ipv4Addr::LOCALHOST, 0).into()).expect("bind listener");
    let sender = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind sender");
    sender
        .send_to(b"bpm:120", listener.local_addr())
        .expect("send first");
    sender
        .send_to(b"play\n", listener.local_addr())
        .expect("send second");

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut received = Vec::new();
    while received.len() < 2 && Instant::now() < deadline {
        received.extend(listener.poll());
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(received, vec!["bpm:120".to_owned(), "play".to_owned()]);
}
