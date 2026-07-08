# ADR 0008: Orca Grid Clock Sync via Cycle-Boundary Re-Publish

- Status: Accepted
- Date: 2026-07-08

## Context

The Orca grid surface (design: `docs/design/orca-surface.md`, spike:
`crates/orpheus-lang/src/orca/`) is a generator: ticking the grid `F` frames
materializes one musical cycle of events. The existing publication path
(`SamplePatternValue::from_events` -> `ReplSession::push_pattern_update` ->
`EngineCommand::LoadPattern`) ships one materialized unit cycle that the audio
engine replays at every cycle boundary. That model assumes cycle-periodic
patterns — but a running grid is generally *not* periodic (a moving `E` does
not return home), so publishing once would loop the first cycle forever.

Two options were identified: (a) re-materialize and re-publish the next cycle
at each engine cycle boundary from the UI side, or (b) add a per-cycle
generator track source (`TrackSource::Generator`) queried by the engine in
`begin_cycle`. Option (b) is an engine-core change that conflicts with the
allocation-free audio-thread rule (pattern queries allocate) and would need
its own mitigation design.

## Decision

Adopt option (a): a UI-side, poll-driven re-publish loop, implemented in
`crates/orpheus-lang/src/orca/publish.rs` with zero engine changes.

### Key design choices

**Boundary detection by polling `TransportSnapshot`.** `OrcaPublisher::poll`
receives `current_cycle_start_frame` from the snapshot the TUI already reads
every 50 ms tick. On the first poll after start, and whenever the cycle start
frame changes, it ticks the grid engine `F` frames (`materialize_cycle`) and
returns a fresh unit-cycle `Vec<Event<SampleEvent>>` for publication. The
engine adopts a published pattern at the *next* boundary, so each batch is
published one cycle ahead of when it sounds; consecutive grid cycles therefore
play back-to-back. Timing stays sample-accurate because event positions are
exact rationals (`frame_span`) converted by the existing scheduler; only the
*publication* moment rides the 50 ms poll, with a full cycle of slack.

**Single-threaded UI-side materialization.** The grid engine, publisher, and
session all live in the TUI's `Rc<RefCell<SharedState>>`. Materialization and
publication happen on the UI thread inside the existing event loop
(`SharedState::poll_orca`); the audio thread is reached only through the
existing lock-free `EngineCommand` queue. No new threads, locks, or
allocations on the audio path.

**The visible grid leads the audio by one cycle.** Materialization advances
the real grid state, so the pane shows the state that will produce the *next*
cycle while the playhead column (derived from
`TransportSnapshot::{current_frame, current_cycle_start_frame,
frames_per_cycle}`) tracks the cycle currently sounding. Edits land in the
next materialization: audible within one to two cycles. A frame-accurate
display copy is deferred until the grid implements `Pattern<T>` directly.

**Note mapping: base-36 value as chromatic semitone offset.** An emitted note
glyph's base-36 value (0-35) repitches the configured sample token (default
`tri`, one of the whitelisted synth primitives) by that many semitones via the
same playback-rate rule as the `pitch` builtin (`2^(value / 12)`), giving a
three-octave chromatic range. Orca's letters-as-note-names convention (with
separate octave ports) is deferred until the full `:` port layout lands.

**Publication reuses the session binding path.** Grid cycles publish under the
reserved binding name `orca` through `ReplSession::publish_sample_events`,
which registers a real `Value::SamplePattern` binding. The grid thereby
inherits mixer routing, track binding, and the transport display for free.
Grid publications bypass the undo history (ADR 0006) deliberately: a running
grid re-publishes every cycle and would flood the snapshot ring.

## Consequences

- No engine changes; the audio-thread allocation-free rule is untouched.
- The grid stops evolving when the TUI event loop stalls longer than a cycle
  (the previous cycle keeps looping — a graceful failure mode) and skips
  cycles if a boundary is missed entirely; acceptable at musical tempos where
  one cycle is on the order of seconds.
- Stopping the grid publishes an empty cycle, so silence also lands exactly on
  a cycle boundary.
- A future `TrackSource::Generator` (option b) remains open; the publisher's
  `materialize_cycle` seam is exactly the function such a track source would
  call, so migration is additive.
