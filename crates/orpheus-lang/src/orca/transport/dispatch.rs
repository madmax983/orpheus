//! The transport dispatcher: a time-ordered action queue routing
//! [`OrcaIoEvent`]s to the UDP, OSC, and MIDI transports.
//!
//! The dispatcher is a pure state machine driven by explicit [`Instant`]s:
//! [`TransportDispatcher::schedule`] queues an event at its deadline and
//! [`TransportDispatcher::run_due`] fires everything due, so all behavior
//! (note-off timing, mono cut, duplicate retrigger, per-family routing) is
//! testable synchronously. The worker thread (`worker.rs`) is a thin loop
//! around these two calls.
//!
//! MIDI note lifecycle mirrors the reference io layer:
//!
//! - **`:` poly (`io/midi.js`)** — note-on at the event deadline, note-off
//!   `length` grid frames later (`length` counts down one per frame in the
//!   reference; zero-length notes press and release in the same pass).
//!   Retriggering a sounding (channel, note) pair releases it first and
//!   supersedes its pending note-off (`push()` releases duplicates).
//! - **`%` mono (`io/mono.js`)** — one active note per channel: a new note
//!   first releases whatever the channel was playing.
//!
//! Send failures are collected as strings for the host's status line and
//! never abort dispatch. `$` command events are *not* interpreted here —
//! command interpretation stays a host concern (`docs/design/orca-surface.md`
//! sections 9.5/13.1) — but as of v7 they are collected at their deadlines
//! ([`TransportDispatcher::take_commands`]) so the host applies them at the
//! same wall-clock moment the other IO fires.
//!
//! v7 also adds MIDI clock out (`io/midi.js` `sendClock*`): while running,
//! the dispatcher emits 0xF8 ticks at one sixth of the grid frame duration
//! (6 ticks per 16th-note frame = 24 PPQN), anchored tick-to-tick on the
//! previous deadline so no cumulative drift accrues (the reference instead
//! re-arms six `setTimeout`s per frame from its UI timer). Starting sends
//! 0xFA, stopping sends 0xFC, and a tempo change retunes the tick period
//! from the next tick.

use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap};
use std::time::{Duration, Instant};

use super::midi::{MidiSink, cc_bytes, note_off_bytes, note_on_bytes, pb_bytes};
use super::net::{OscTransport, UdpTransport};
use super::{ScheduledIoEvent, TransportConfig};
use crate::orca::engine::{MidiNote, OrcaIoEvent};
use crate::orca::publish::midi_note_id;

/// MIDI real-time bytes for clock out (`io/midi.js` `sendClock*`).
const CLOCK_TICK: u8 = 0xF8;
/// See [`CLOCK_TICK`].
const CLOCK_START: u8 = 0xFA;
/// See [`CLOCK_TICK`].
const CLOCK_STOP: u8 = 0xFC;

/// Clock ticks per grid frame: one frame is a 16th note and MIDI clock is
/// 24 PPQN, so 6 ticks per frame (`io/midi.js` `frameFrag = frameTime / 6`).
const CLOCK_TICKS_PER_FRAME: u32 = 6;

/// Floor for the clock tick period, guarding a degenerate (zero) frame
/// duration from turning the tick rescheduler into a busy loop.
const MIN_CLOCK_TICK: Duration = Duration::from_micros(500);

/// An active mono voice: the note-off bytes to send when it is cut or
/// expires, and the generation guarding its pending note-off action.
#[derive(Clone, Copy, Debug)]
struct MonoVoice {
    generation: u64,
    off: [u8; 3],
}

/// The running MIDI clock: the tick period (grid frame duration / 6) and
/// the generation guarding pending tick actions (stopping or restarting
/// the clock strands the old generation's ticks).
#[derive(Clone, Copy, Debug)]
struct ClockState {
    generation: u64,
    tick: Duration,
}

/// The clock tick period for a grid frame duration.
fn clock_tick_period(frame_duration: Duration) -> Duration {
    (frame_duration / CLOCK_TICKS_PER_FRAME).max(MIN_CLOCK_TICK)
}

/// A queued action, fired when `due` arrives. `sequence` keeps same-instant
/// actions in submission order (grid scan order; note-ons before their
/// zero-length note-offs).
#[derive(Debug)]
struct QueuedAction {
    due: Instant,
    sequence: u64,
    action: Action,
}

impl PartialEq for QueuedAction {
    fn eq(&self, other: &Self) -> bool {
        self.due == other.due && self.sequence == other.sequence
    }
}

impl Eq for QueuedAction {}

impl PartialOrd for QueuedAction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedAction {
    fn cmp(&self, other: &Self) -> Ordering {
        self.due
            .cmp(&other.due)
            .then_with(|| self.sequence.cmp(&other.sequence))
    }
}

#[derive(Debug)]
enum Action {
    /// A grid IO event firing at its deadline.
    Io {
        io: OrcaIoEvent,
        frame_duration: Duration,
    },
    /// A pending `:` note-off; stale (superseded) when the generation no
    /// longer matches the sounding note's.
    PolyOff {
        channel: u8,
        note_id: u8,
        generation: u64,
        bytes: [u8; 3],
    },
    /// A pending `%` note-off for whatever the channel is sounding, guarded
    /// by generation like `PolyOff`.
    MonoOff { channel: u8, generation: u64 },
    /// One MIDI clock tick (0xF8); fires and reschedules itself one tick
    /// period later while its generation still matches the running clock.
    ClockTick { generation: u64 },
}

/// Routes scheduled [`OrcaIoEvent`]s to the UDP, OSC, and MIDI transports
/// at their deadlines. See the module docs for semantics.
pub struct TransportDispatcher {
    midi: Option<Box<dyn MidiSink>>,
    cc_offset: u8,
    udp: UdpTransport,
    osc: OscTransport,
    /// Sounding `:` notes, keyed by (channel, note id) -> generation.
    poly: HashMap<(u8, u8), u64>,
    /// Sounding `%` notes, one per channel.
    mono: HashMap<u8, MonoVoice>,
    /// The running MIDI clock, if any.
    clock: Option<ClockState>,
    /// `$` command strings collected at their deadlines, awaiting the
    /// host's [`Self::take_commands`].
    pending_commands: Vec<String>,
    queue: BinaryHeap<Reverse<QueuedAction>>,
    next_sequence: u64,
    next_generation: u64,
}

impl std::fmt::Debug for TransportDispatcher {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TransportDispatcher")
            .field("has_midi", &self.midi.is_some())
            .field("pending", &self.queue.len())
            .finish_non_exhaustive()
    }
}

impl TransportDispatcher {
    /// A dispatcher with no MIDI sink attached and sockets aimed at the
    /// configured targets (bound lazily on first send).
    #[must_use]
    pub fn new(config: &TransportConfig) -> Self {
        Self {
            midi: None,
            cc_offset: config.cc_offset,
            udp: UdpTransport::new(config.udp_target),
            osc: OscTransport::new(config.osc_target),
            poly: HashMap::new(),
            mono: HashMap::new(),
            clock: None,
            pending_commands: Vec::new(),
            queue: BinaryHeap::new(),
            next_sequence: 0,
            next_generation: 0,
        }
    }

    /// Attaches (or detaches, with `None`) the MIDI sink. Note events
    /// dispatched without a sink are dropped silently, matching the
    /// reference's "No midi output!" no-op.
    pub fn set_midi(&mut self, sink: Option<Box<dyn MidiSink>>) {
        self.midi = sink;
    }

    /// Re-aims the UDP transport.
    pub fn set_udp_target(&mut self, target: std::net::SocketAddr) {
        self.udp.set_target(target);
    }

    /// Re-aims the OSC transport.
    pub fn set_osc_target(&mut self, target: std::net::SocketAddr) {
        self.osc.set_target(target);
    }

    /// Queues one event for its deadline.
    pub fn schedule(&mut self, event: ScheduledIoEvent) {
        let ScheduledIoEvent {
            fire_at,
            frame_duration,
            io,
        } = event;
        self.push_action(fire_at, Action::Io { io, frame_duration });
    }

    /// The earliest pending deadline, for the worker's sleep.
    #[must_use]
    pub fn next_due(&self) -> Option<Instant> {
        self.queue.peek().map(|Reverse(action)| action.due)
    }

    /// Whether any action is still queued.
    #[must_use]
    pub fn has_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Drains the `$` command strings that fired since the last call, in
    /// deadline order, for the host's command interpreter.
    #[must_use]
    pub fn take_commands(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending_commands)
    }

    /// Whether the MIDI clock is currently running (ticks flowing).
    #[must_use]
    pub const fn clock_running(&self) -> bool {
        self.clock.is_some()
    }

    /// Starts MIDI clock out: sends 0xFA and begins 0xF8 ticks at one
    /// sixth of `frame_duration`, the first at `now` (`io/midi.js`
    /// `sendClockStart` + per-frame `sendClock`). Restarting an already
    /// running clock re-anchors it (and re-sends 0xFA). Returns transport
    /// errors, matching [`Self::run_due`].
    pub fn clock_start(&mut self, now: Instant, frame_duration: Duration) -> Vec<String> {
        let mut errors = Vec::new();
        let generation = self.next_generation;
        self.next_generation += 1;
        self.clock = Some(ClockState {
            generation,
            tick: clock_tick_period(frame_duration),
        });
        self.send_midi(&[CLOCK_START], &mut errors);
        self.push_action(now, Action::ClockTick { generation });
        errors
    }

    /// Stops MIDI clock out: sends 0xFC and strands pending ticks
    /// (`io/midi.js` `sendClockStop`). A stopped clock is a no-op.
    pub fn clock_stop(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        if self.clock.take().is_some() {
            self.send_midi(&[CLOCK_STOP], &mut errors);
        }
        errors
    }

    /// Retunes the running clock's tick period to `frame_duration / 6`,
    /// effective from the next tick. A stopped clock is unaffected.
    pub fn set_clock_frame_duration(&mut self, frame_duration: Duration) {
        if let Some(clock) = &mut self.clock {
            clock.tick = clock_tick_period(frame_duration);
        }
    }

    /// Fires every action due at or before `now`, in (deadline, submission)
    /// order. Returns the transport errors encountered, one message each.
    pub fn run_due(&mut self, now: Instant) -> Vec<String> {
        let mut errors = Vec::new();
        while self
            .queue
            .peek()
            .is_some_and(|Reverse(action)| action.due <= now)
        {
            if let Some(Reverse(queued)) = self.queue.pop() {
                self.fire(queued, &mut errors);
            }
        }
        errors
    }

    /// Releases every sounding note immediately (shutdown hygiene: devices
    /// must not be left with hanging notes). Pending non-note actions are
    /// discarded.
    pub fn flush_note_offs(&mut self) -> Vec<String> {
        let mut errors = Vec::new();
        let poly: Vec<_> = self.poly.drain().collect();
        for ((channel, note_id), _) in poly {
            // The stored generation does not carry the velocity byte; the
            // reference releases with the item's own velocity, but for a
            // shutdown flush a zero-velocity note-off is universally safe.
            self.send_midi(&note_off_bytes(channel, note_id, 0), &mut errors);
        }
        let mono: Vec<_> = self.mono.drain().collect();
        for (_, voice) in mono {
            self.send_midi(&voice.off, &mut errors);
        }
        // A running clock stops cleanly too (devices treat a vanished
        // clock as a stall; 0xFC tells them the transport stopped).
        errors.extend(self.clock_stop());
        self.queue.clear();
        errors
    }

    fn push_action(&mut self, due: Instant, action: Action) {
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.queue.push(Reverse(QueuedAction {
            due,
            sequence,
            action,
        }));
    }

    fn fire(&mut self, queued: QueuedAction, errors: &mut Vec<String>) {
        match queued.action {
            Action::Io { io, frame_duration } => {
                self.fire_io(&io, queued.due, frame_duration, errors);
            }
            Action::PolyOff {
                channel,
                note_id,
                generation,
                bytes,
            } => {
                if self.poly.get(&(channel, note_id)) == Some(&generation) {
                    self.poly.remove(&(channel, note_id));
                    self.send_midi(&bytes, errors);
                }
            }
            Action::MonoOff {
                channel,
                generation,
            } => {
                let expired = self
                    .mono
                    .get(&channel)
                    .is_some_and(|voice| voice.generation == generation);
                if expired && let Some(voice) = self.mono.remove(&channel) {
                    self.send_midi(&voice.off, errors);
                }
            }
            Action::ClockTick { generation } => {
                let Some(clock) = self.clock else {
                    return;
                };
                if clock.generation != generation {
                    return;
                }
                self.send_midi(&[CLOCK_TICK], errors);
                // Anchor the next tick on this one's deadline (not `now`),
                // so late wakeups never accumulate into drift.
                self.push_action(queued.due + clock.tick, Action::ClockTick { generation });
            }
        }
    }

    fn fire_io(
        &mut self,
        io: &OrcaIoEvent,
        at: Instant,
        frame_duration: Duration,
        errors: &mut Vec<String>,
    ) {
        match io {
            OrcaIoEvent::Midi(note) => self.poly_note(*note, at, frame_duration, errors),
            OrcaIoEvent::MidiMono(note) => self.mono_note(*note, at, frame_duration, errors),
            OrcaIoEvent::MidiCc {
                channel,
                knob,
                value,
            } => {
                self.send_midi(&cc_bytes(*channel, *knob, *value, self.cc_offset), errors);
            }
            OrcaIoEvent::MidiPb { channel, lsb, msb } => {
                self.send_midi(&pb_bytes(*channel, *lsb, *msb), errors);
            }
            OrcaIoEvent::Udp(message) => {
                if let Err(error) = self.udp.send(message) {
                    errors.push(format!("orca UDP send failed: {error}"));
                }
            }
            OrcaIoEvent::Osc { path, args } => {
                if let Err(error) = self.osc.send(*path, args) {
                    errors.push(format!("orca OSC send failed: {error}"));
                }
            }
            // `$` commands stay uninterpreted here (section 9.5); they are
            // collected at their deadlines for the host's interpreter
            // (design doc section 13.1), which drains them via
            // `take_commands`.
            OrcaIoEvent::Command(command) => self.pending_commands.push(command.clone()),
        }
    }

    /// `:` — polyphonic note. The reference drops glyphs outside the
    /// transpose table at send time; a duplicate (channel, note) is
    /// released first and its pending note-off superseded.
    fn poly_note(
        &mut self,
        note: MidiNote,
        at: Instant,
        frame_duration: Duration,
        errors: &mut Vec<String>,
    ) {
        let Some(note_id) = midi_note_id(note.note, note.octave) else {
            return;
        };
        let key = (note.channel, note_id);
        let off = note_off_bytes(note.channel, note_id, note.velocity);
        if self.poly.contains_key(&key) {
            // Duplicate retrigger (`midi.js` `push()`): release before
            // re-pressing. The reference releases with the *new* item's
            // velocity, since only the velocity byte can differ.
            self.send_midi(&off, errors);
        }
        self.send_midi(&note_on_bytes(note.channel, note_id, note.velocity), errors);
        let generation = self.next_generation;
        self.next_generation += 1;
        self.poly.insert(key, generation);
        self.push_action(
            at + frame_duration * u32::from(note.length),
            Action::PolyOff {
                channel: note.channel,
                note_id,
                generation,
                bytes: off,
            },
        );
    }

    /// `%` — monophonic note: cuts whatever the channel is sounding
    /// (`mono.js` `push()` releases `stack[channel]` first).
    fn mono_note(
        &mut self,
        note: MidiNote,
        at: Instant,
        frame_duration: Duration,
        errors: &mut Vec<String>,
    ) {
        let Some(note_id) = midi_note_id(note.note, note.octave) else {
            return;
        };
        if let Some(previous) = self.mono.remove(&note.channel) {
            self.send_midi(&previous.off, errors);
        }
        self.send_midi(&note_on_bytes(note.channel, note_id, note.velocity), errors);
        let generation = self.next_generation;
        self.next_generation += 1;
        self.mono.insert(
            note.channel,
            MonoVoice {
                generation,
                off: note_off_bytes(note.channel, note_id, note.velocity),
            },
        );
        self.push_action(
            at + frame_duration * u32::from(note.length),
            Action::MonoOff {
                channel: note.channel,
                generation,
            },
        );
    }

    fn send_midi(&mut self, bytes: &[u8], errors: &mut Vec<String>) {
        if let Some(sink) = self.midi.as_mut()
            && let Err(error) = sink.send(bytes)
        {
            errors.push(format!("orca MIDI send failed: {error}"));
        }
    }
}
