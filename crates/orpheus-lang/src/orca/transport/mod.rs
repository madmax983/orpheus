//! Real IO transports for the Orca grid surface (v6).
//!
//! v3 gave the IO operator family (`:` `%` `!` `?` `;` `=` `$`) typed
//! [`OrcaIoEvent`] payloads with no transport attached; this module attaches
//! the transports. Wire behavior is pinned against the reference
//! implementation's io layer (`hundredrabbits/Orca` main branch:
//! `core/io/udp.js`, `core/io/osc.js`, `core/io/midi.js`, `core/io/cc.js`,
//! `core/io/mono.js`):
//!
//! - **UDP (`;`)** — the raw message string as one datagram, default target
//!   `127.0.0.1:49161` (the reference's *output* port; `49160` is its input
//!   listener, which Orpheus does not implement).
//! - **OSC (`=`)** — an OSC message with address `/<path>` and one int32
//!   argument per arg glyph (its base-36 value, matching
//!   `orca.valueOf`), default target `127.0.0.1:49162`.
//! - **MIDI (`:` `%` `!` `?`)** — note-on/note-off pairs with the note-off
//!   scheduled `length` grid frames after the note-on, control change with
//!   the reference's `+64` knob offset, and raw pitch-bend bytes, sent to a
//!   [`MidiSink`] (a real `midir` port, or a recording mock in tests).
//! - **`$` commands** (v7) are collected at their wall-clock deadlines and
//!   handed back to the host ([`TransportHandle::poll_commands`]), which
//!   interprets them with `orca::parse_command` — interpretation stays a
//!   host concern, off the IO thread.
//!
//! v7 additions on the same worker: **MIDI clock out** (0xFA/0xF8/0xFC,
//! six ticks per grid frame = 24 PPQN, `io/midi.js` `sendClock*`),
//! host-configured via `:orca midi clock on|off` and off by default; and
//! the **UDP command listener** ([`UdpCommandListener`], reference input
//! port 49160), whose received datagrams feed the same command
//! interpreter.
//!
//! # Threading
//!
//! Transports never run on the audio thread (which stays allocation-free
//! and lock-free) and never block the TUI tick. [`TransportHandle::spawn`]
//! starts one dedicated `orca-io` worker thread owning a
//! [`TransportDispatcher`]; the TUI feeds it [`ScheduledIoEvent`]s over an
//! `mpsc` channel and the worker fires each event at its wall-clock
//! deadline (see ADR 0010). The dispatcher itself is a pure, synchronous
//! state machine driven by explicit [`Instant`]s, so every behavior is
//! testable without threads or hardware.
//!
//! # Timing
//!
//! Grid cycles are materialized one cycle ahead of playback (ADR 0009), so
//! the host converts each event's cycle-relative frame into a wall-clock
//! deadline via [`cycle_schedule`] and ships `fire_at` with the event.
//! MIDI note-offs are scheduled `length * frame_duration` after the
//! note-on, mirroring the reference's per-frame length countdown.

mod dispatch;
mod midi;
mod net;
mod worker;

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

use thiserror::Error;

use super::engine::OrcaIoEvent;

pub use dispatch::TransportDispatcher;
pub use midi::{
    MidiSink, MidirSink, RecordingMidiSink, cc_bytes, connect_midi_output, midi_output_names,
    note_off_bytes, note_on_bytes, pb_bytes, velocity_byte,
};
pub use net::{OscTransport, UdpCommandListener, UdpTransport};
pub use worker::{TransportCommand, TransportHandle};

/// Default UDP target: the reference client's default *output* port
/// (`udp.js` `selectOutput(port = 49161)`; `49160` is its input listener,
/// see [`DEFAULT_UDP_LISTEN`]).
pub const DEFAULT_UDP_TARGET: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 49161));

/// Default UDP listener address: the reference client's input port
/// (`udp.js` `selectInput(port = 49160)`), bound on loopback.
///
/// The reference binds all interfaces; `:orca listen <host:port>` opts
/// into a wider bind explicitly.
pub const DEFAULT_UDP_LISTEN: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 49160));

/// Default OSC target (`osc.js` `options.default = 49162`).
pub const DEFAULT_OSC_TARGET: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 49162));

/// Default MIDI CC knob offset (`cc.js` `this.offset = 64`): the byte sent
/// as the controller number is `offset + knob`.
pub const DEFAULT_CC_OFFSET: u8 = 64;

/// Transport targets and MIDI scaling knobs, host-configurable via the
/// `:orca` TUI command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportConfig {
    /// Destination for `;` UDP datagrams.
    pub udp_target: SocketAddr,
    /// Destination for `=` OSC messages.
    pub osc_target: SocketAddr,
    /// Added to the `!` knob value to form the CC controller byte.
    pub cc_offset: u8,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            udp_target: DEFAULT_UDP_TARGET,
            osc_target: DEFAULT_OSC_TARGET,
            cc_offset: DEFAULT_CC_OFFSET,
        }
    }
}

/// Failures raised by transport sends and MIDI port management. Dispatch
/// errors are reported (as status messages) and never abort the worker.
#[derive(Debug, Error)]
pub enum TransportError {
    /// A socket bind or send failed.
    #[error("socket error: {0}")]
    Socket(#[from] std::io::Error),
    /// An OSC message failed to encode.
    #[error("OSC encoding error: {0}")]
    OscEncode(String),
    /// The MIDI subsystem failed to initialize.
    #[error("failed to initialize MIDI output subsystem: {0}")]
    MidiInit(String),
    /// No MIDI output port matched the requested name.
    #[error("no MIDI output port named `{0}`")]
    MidiPortNotFound(String),
    /// Connecting to a MIDI output port failed.
    #[error("failed to connect to MIDI output `{port}`: {reason}")]
    MidiConnect {
        /// The port that was being connected.
        port: String,
        /// The backend's failure description.
        reason: String,
    },
    /// Sending bytes to a MIDI output failed.
    #[error("MIDI send error: {0}")]
    MidiSend(String),
}

/// One IO event stamped with its wall-clock deadline.
#[derive(Clone, Debug)]
pub struct ScheduledIoEvent {
    /// When the event fires.
    pub fire_at: Instant,
    /// Wall-clock duration of one grid frame at the tempo the event was
    /// scheduled under; MIDI note-offs fire `length` frames after
    /// `fire_at`.
    pub frame_duration: Duration,
    /// The typed IO payload.
    pub io: OrcaIoEvent,
}

/// Beats per musical cycle, mirroring `orpheus-dsp`'s `BEATS_PER_CYCLE`.
const BEATS_PER_CYCLE: f64 = 4.0;

/// Fallback tempo when the snapshot reports a degenerate BPM.
const FALLBACK_TEMPO_BPM: f64 = 120.0;

/// Maps a freshly materialized grid cycle onto wall-clock time, returning
/// the cycle's start instant and the duration of one grid frame.
///
/// Cycles are materialized one cycle ahead: the publisher polls at an
/// engine cycle boundary and the batch it returns plays from the *next*
/// boundary (ADR 0009). The next boundary lies one cycle minus the elapsed
/// in-cycle fraction ahead of `now`; the in-cycle fraction comes from the
/// transport snapshot's frame counters, so no sample rate is needed. While
/// the transport is stopped (or the engine clock is degenerate) the cycle
/// is anchored at `now` — events fire immediately in grid order.
#[must_use]
#[allow(clippy::cast_precision_loss)] // frame counts are far below 2^52
pub fn cycle_schedule(
    now: Instant,
    tempo_bpm: f32,
    current_frame: u64,
    cycle_start_frame: u64,
    engine_frames_per_cycle: u64,
    grid_frames_per_cycle: u64,
    is_playing: bool,
) -> (Instant, Duration) {
    let bpm = f64::from(tempo_bpm);
    let bpm = if bpm.is_finite() && bpm > 0.0 {
        bpm
    } else {
        FALLBACK_TEMPO_BPM
    };
    let cycle_seconds = BEATS_PER_CYCLE * 60.0 / bpm;
    let frame_duration =
        Duration::from_secs_f64(cycle_seconds / grid_frames_per_cycle.max(1) as f64);
    if !is_playing || engine_frames_per_cycle == 0 {
        return (now, frame_duration);
    }
    let next_boundary = cycle_start_frame.saturating_add(engine_frames_per_cycle);
    let remaining_frames = next_boundary.saturating_sub(current_frame);
    let remaining_fraction =
        (remaining_frames as f64 / engine_frames_per_cycle as f64).clamp(0.0, 1.0);
    let start = now + Duration::from_secs_f64(cycle_seconds * remaining_fraction);
    (start, frame_duration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_schedule_anchors_the_next_boundary_and_frame_duration() {
        let now = Instant::now();
        // 120 BPM, 4 beats per cycle -> 2 s cycles. Engine halfway through
        // the current cycle -> next boundary in 1 s. 16 grid frames per
        // cycle -> 125 ms per frame.
        let (start, frame) = cycle_schedule(now, 120.0, 48_000, 0, 96_000, 16, true);
        assert_eq!(start - now, Duration::from_secs(1));
        assert_eq!(frame, Duration::from_millis(125));
    }

    #[test]
    fn cycle_schedule_fires_immediately_while_stopped() {
        let now = Instant::now();
        let (start, frame) = cycle_schedule(now, 120.0, 48_000, 0, 96_000, 16, false);
        assert_eq!(start, now);
        assert_eq!(frame, Duration::from_millis(125));
    }

    #[test]
    fn cycle_schedule_survives_degenerate_clocks() {
        let now = Instant::now();
        // Zero BPM falls back to 120; zero engine frames-per-cycle anchors
        // at `now`; zero grid frames-per-cycle avoids dividing by zero.
        let (start, frame) = cycle_schedule(now, 0.0, 0, 0, 0, 0, true);
        assert_eq!(start, now);
        assert_eq!(frame, Duration::from_secs(2));
    }

    #[test]
    fn cycle_schedule_clamps_a_stale_snapshot() {
        let now = Instant::now();
        // A snapshot from before the boundary crossing (current frame past
        // the next boundary) clamps to zero remaining time.
        let (start, _) = cycle_schedule(now, 120.0, 200_000, 0, 96_000, 16, true);
        assert_eq!(start, now);
    }

    #[test]
    fn default_config_matches_the_reference_ports() {
        let config = TransportConfig::default();
        assert_eq!(config.udp_target.port(), 49_161);
        assert_eq!(config.osc_target.port(), 49_162);
        assert_eq!(config.cc_offset, 64);
    }
}
