# ADR 0010: Dedicated IO Thread for Orca Grid Transports

- Status: Accepted
- Date: 2026-07-08

## Context

Orca surface v3 gave the grid's IO operator family (`:` `%` `!` `?` `;`
`=` `$`) typed `OrcaIoEvent` payloads with no transport attached: MIDI-note
events reach the internal sampler through the publish bridge (ADR 0008/0009),
but CC, pitch bend, UDP, OSC, and command events accumulated per tick with no
consumer. v6 attaches real transports — UDP and OSC sockets, and a `midir`
MIDI output port — which raises the question of *where* those sends run.

Constraints:

1. **Not the audio thread.** The audio path must stay allocation-free and
   lock-free (workspace rule); socket sends and MIDI writes are blocking
   syscalls that allocate.
2. **Not the TUI tick.** The event loop polls the transport at 50 ms; a slow
   DNS-less-but-still-blocking `send_to` or a wedged MIDI driver must not
   stall rendering or input handling.
3. **Timing.** Grid cycles are materialized one full cycle ahead of playback
   (ADR 0009), so events cannot fire when drained — they carry deadlines up
   to two cycles in the future, and MIDI note-offs fire `length` grid frames
   after their note-on (reference `io/midi.js` counts length down once per
   frame).
4. **Testability.** Every wire behavior must be assertable without hardware
   or timers.

## Decision

One dedicated worker thread (`orca-io`), owned by a `TransportHandle` on the
TUI's `SharedState`, hosting a synchronous `TransportDispatcher`
(`crates/orpheus-lang/src/orca/transport/`):

- The dispatcher is a **pure, instant-driven state machine**: a
  `BinaryHeap` of (deadline, submission-sequence)-ordered actions plus the
  sounding-note state (poly map keyed by channel+note, mono map keyed by
  channel, generation counters to invalidate superseded note-offs).
  `schedule()` queues, `run_due(now)` fires; tests drive it with synthetic
  `Instant`s and a recording `MidiSink`.
- The worker is a thin loop: sleep until the next queued deadline (or an
  idle tick), `recv_timeout` for commands (`Schedule`, `SetMidi`,
  `SetUdpTarget`, `SetOscTarget`), fire what is due, report send errors
  over a status channel that the TUI drains into its status line.
- The host converts cycle-relative frame stamps to wall-clock deadlines at
  scheduling time (`cycle_schedule`: next-boundary fraction from the
  `TransportSnapshot` frame counters × `240 / bpm` seconds per cycle), so
  the worker needs no knowledge of the engine clock.
- **Shutdown hygiene:** dropping the handle closes the command channel; the
  worker flushes note-offs for every sounding note (devices must not be
  left hanging), discards other pending events, and exits; the drop joins
  it.
- MIDI hardware hides behind the `MidiSink` trait (`MidirSink` real,
  `RecordingMidiSink` in tests); the sink is constructed on the TUI thread
  (synchronous error reporting for `:orca midi connect`) and shipped to the
  worker, which is sound because `midir::MidiOutputConnection` is `Send`.

## Alternatives considered

- **Send on the TUI tick when draining events.** Simplest, but violates
  constraint 2, and cycle-ahead materialization would make everything fire
  one cycle early and bunched at boundaries — audibly wrong for UDP/OSC
  consumers and useless for MIDI note-off pacing.
- **A thread per transport family.** No benefit: the families share the
  timing queue (a mono cut must order against the note it cuts), and one
  mostly-sleeping thread is cheaper than three.
- **Async runtime.** A tokio/async-io dependency for three blocking sends
  per frame at most is out of proportion for this workspace, which has no
  async anywhere.
- **Engine-side scheduling (audio callback emits into a ring drained by the
  IO thread).** Sample-accurate, but couples `orpheus-dsp` to Orca IO types
  and adds audio-thread work; the wall-clock scheme's error is bounded by
  snapshot-poll jitter (≤ one 50 ms tick), which is comparable to the
  reference client's own `setTimeout`-driven jitter.

## Consequences

- Non-note IO events now reach real sockets/devices with frame-accurate
  pacing; the audio path is untouched.
- Grid IO timing derives from the wall clock between boundary polls, so a
  tempo change mid-cycle shifts already-scheduled events by up to one cycle
  until the next materialization — accepted (the reference client has the
  same property between its clock updates).
- Every `SharedState` owns one extra parked thread; it exits with the
  state's drop (tests included, via channel disconnect + join).
- Offline export ignores transports entirely (as it ignores the generator
  source, ADR 0009); recording IO output is future work.
