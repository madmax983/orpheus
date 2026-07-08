# ADR 0009: Engine-Side Generator Track Source with Cross-Cycle Sustain

- Status: Accepted (amends ADR 0008)
- Date: 2026-07-08

## Context

ADR 0008 made the Orca grid audible with zero engine changes: a UI-side,
poll-driven loop re-published the next materialized grid cycle as a whole
pattern binding (`ReplSession::publish_sample_events` ->
`EngineCommand::LoadPattern` / `SwapRoutingSnapshot`) at every engine cycle
boundary. That worked, but with two structural costs:

1. **Notes clamped at cycle boundaries.** The scheduler derived trigger
   durations from the event's clipped `part`, and each boundary replaced the
   published pattern wholesale, so a note whose length crossed the cycle end
   lost its tail (`docs/design/orca-surface.md` section 10.2, a documented
   deviation from reference Orca).
2. **Per-cycle routing churn.** Every grid cycle re-compiled and re-adopted a
   full `RoutingSnapshot` (rebuilding plugin processors and bus-effect state
   preservation logic each boundary) and re-inserted the `orca` binding, just
   to deliver one cycle of events.

ADR 0008 explicitly deferred the alternative: a first-class engine generator
source. This ADR adopts it.

## Decision

Add a generator track source to `orpheus-dsp` and ship pre-materialized cycle
buffers to it over the existing lock-free command ring; derive scheduled
trigger durations from the event's `whole` extent so clipped events sustain
across cycle boundaries.

### The dsp seam (generic, Orca-agnostic)

- **`TrackSource::Generator(GeneratorId)`** (`routing.rs`): a track whose
  events are not stored in the snapshot but delivered per cycle. `GeneratorId`
  indexes one of `MAX_GENERATORS` (8) fixed engine slots.
- **`EngineCommand::PushGeneratorCycle(GeneratorCycle)`** (`command.rs`):
  `GeneratorCycle` carries a `GeneratorId` plus a `Box<[Event<SampleTrigger>]>`
  materialized and boxed off the audio thread. Latest delivery wins if two
  arrive within one cycle.
- **Engine slots** (`engine.rs`): `EngineCore` preallocates two fixed-size
  tables at construction, `generator_pending` (buffers delivered but not yet
  adopted) and `generator_active` (the buffer each slot schedules). At each
  `begin_cycle` the engine moves any pending buffer into the active slot, then
  schedules the active buffer for every `Generator` track through the same
  `Scheduler::schedule_cycle_events` path used by `SamplePattern` tracks.
- **Starvation degrades to looping.** When no fresh buffer arrived by the
  boundary, the previous active buffer is re-scheduled: a stalled UI repeats
  the last grid cycle instead of going silent — the same graceful failure mode
  ADR 0008 chose. Stopping is therefore explicit: deliver an empty buffer,
  which loops silence from the next boundary.
- **Cross-cycle sustain** (`scheduler.rs`): `duration_frames_for_event` now
  runs from the trigger frame (`part.start`) to `max(part.end, whole.end)`.
  An event clipped at the cycle end keeps its unclipped extent in `whole`
  (Tidal semantics, already produced by the Orca bridge in v4), so its voice
  sounds past the boundary; voices were never cut at boundaries, only their
  scheduled durations were. Events clipped at the window *start* are
  unaffected (duration always runs forward from the trigger). Durations
  exceeding `u32` frames saturate instead of erroring the audio thread. This
  applies to every track source, fixing the same truncation for ordinary
  patterns whose events are clipped by `query_unit`.

### The lang glue (Orca-specific)

- `ReplSession::start_generator_source(name, id, events)` registers the
  binding name as generator-backed in `MixerState` (snapshot compiles resolve
  it to `TrackSource::Generator`, whether on the compatibility main track or
  an explicitly bound track), inserts a display binding so the grid appears in
  binding summaries, enqueues one routing snapshot, and delivers the first
  cycle. `push_generator_cycle` maps `Event<SampleEvent>` batches to triggers
  and enqueues them; `stop_generator_source` delivers an empty cycle.
- The Orca grid claims slot `ORCA_GENERATOR_ID` (0). `OrcaPublisher` survives
  as the *driver*: it still detects boundaries by polling `TransportSnapshot`
  from the 50 ms TUI tick and materializes the next cycle one cycle ahead;
  only the transport changed. `SharedState::toggle_orca_running` /`poll_orca`
  now ship batches to the generator slot instead of re-publishing a binding.
- A fresh engine (frame 0) primes generator routing immediately, matching
  `LoadPattern`'s start behavior, so the first grid cycle is audible without
  waiting a full cycle.

### Threading model and allocation/lock analysis of the audio path

Materialization stays where ADR 0008 put it: on the UI thread, inside the
existing event loop, one cycle ahead of playback. Nothing new touches the
audio thread except the two fixed slot tables:

- **Per-frame render path: unchanged.** `render_into_interleaved` still only
  pops due triggers, mixes voices, and advances the frame counter — no
  allocation, no locks, no grid code.
- **Cycle-boundary path: strictly less work than before.** Adopting a
  generator buffer is a boxed-slice move into a preallocated slot plus one
  boundary-time drop of the retired buffer — the same adoption discipline the
  engine already uses for `pending_routing` and `pending_sample_bank` swaps.
  The v4 path additionally rebuilt the whole routing snapshot (plugin
  processor construction, bus-state migration, mix-buffer resizing) every
  cycle; that now happens only when routing actually changes.
- **No locks anywhere new**: buffers travel over the existing rtrb SPSC ring;
  the slot tables are owned exclusively by the audio thread.
- The grid tick itself (which allocates event vectors and scans the grid)
  never runs on the audio thread.

### Why not an engine-side generator trait?

The rejected shape was `TrackSource::Generator(Box<dyn CycleGenerator>)`
invoked inside `begin_cycle`. It fails three ways:

1. **Allocation/compute on the audio thread.** The Orca tick allocates per
   frame (event vectors, operator scratch) and its cost scales with grid area
   times frames-per-cycle — unbounded work inside a real-time callback, in
   direct conflict with the workspace's audio-thread rule. Making the tick
   allocation-free would mean rewriting the grid engine around preallocated
   arenas for no musical benefit.
2. **Dependency inversion.** The grid engine lives in `orpheus-lang`;
   `orpheus-dsp` must not depend on it. A trait object shipped over the ring
   would smuggle lang code onto the audio thread. Pre-materialized buffers
   keep `orpheus-dsp` generic: it knows nothing about Orca, only about "one
   cycle of events per boundary".
3. **Determinism and editability.** UI-side materialization means grid edits,
   status display, and the engine state all live on one thread with no
   synchronization questions; the audio thread consumes immutable data.

### Migration / fallback story for `OrcaPublisher`

- `OrcaPublisher` (boundary detection + `materialize_cycle`) is retained
  unchanged as the cycle driver; all its existing tests stand.
- The per-cycle *re-publish* transport (`publish_sample_events` per boundary)
  is retired from the TUI but remains a supported fallback for hosts without
  generator wiring (headless scripts, future embeddings); it plays correctly
  except that cross-cycle note tails are re-clipped by `query_unit` on the
  round-trip through a pattern binding.
- `publish_sample_events` itself is unchanged and still the right API for
  one-shot publications.

## Consequences

- Note lengths sustain across cycle boundaries end-to-end (reference Orca
  behavior); the section 10.2 limitation is resolved.
- Grid playback no longer recompiles routing or touches the binding table per
  cycle; the `orca` binding is a display artifact updated at start/stop.
- Offline rendering (`offline.rs`) treats generator tracks as silent: their
  buffers live in the real-time engine, not the snapshot. Exporting a grid
  performance is future work (record the delivered buffers).
- The whole-extent duration rule changes one general behavior: a looping
  pattern event clipped at the cycle end now rings into the next cycle each
  time it re-triggers (overlapping its next incarnation) instead of being
  cut. This matches the event's declared extent and Tidal's semantics.
- Stopping the grid still silences at the next boundary; voices already
  sounding ring out their full note length (previously they were cut at the
  boundary along with everything else).
- `MAX_GENERATORS` fixes the slot table at 8; ids outside the range are
  rejected as engine errors at delivery time.
