# 0012. Per-Note Voice Parameter Breakpoint Automation

Date: 2026-07-09

## Status

Accepted

## Context

The ADR 0010 addendum gave patterns per-note control of graph voice bodies:
`melody |> p1(300 6000)` stamps each note's `p1..p4` as plain `f32` fields at
trigger time, held for the note. That left a gap (parity roadmap: "audio-rate
pattern control of voice parameters"): a control pattern with *sub-note*
structure — `p1(segment(8, rand) |> range(200, 2000))` under a held note —
could not move a parameter DURING the note. Worse, the generic control-pattern
machinery (`apply_control_pattern`) fragments source events at control
boundaries, so sub-note `p1` structure chopped one held note into several
retriggered ones.

Two architectural facts constrain the design:

1. **The engine never sees patterns.** Render-time inputs are fully baked
   `Event<SampleTrigger>` buffers inside routing snapshots; the audio thread
   cannot re-query a pattern per block. Any intra-note control data must
   therefore ship *with the trigger*, computed on the non-RT query side.
2. **The audio thread is allocation-free per note.** Whatever ships must land
   in fixed-size storage inside the pooled note structures.

For scope calibration: Tidal's continuous controls re-sample per *event*, and
SuperDirt applies them per *trigger* — literal audio-rate pattern evaluation
is beyond Tidal parity and would require pattern queries on or near the audio
thread.

## Decision

**Query-side breakpoint automation with fixed-capacity, interpolated
playback.**

- **Where breakpoints are computed.** In `orpheus-lang`'s query path
  (`apply_voice_param_pattern`, value.rs): the `p1..p4` controls no longer
  fragment notes. Each source event keeps its span and `whole`; the control
  pattern is sampled over the event's own extent (trigger point to the end of
  the unclipped `whole` — the scheduler's duration semantics). The control's
  value at the extent start becomes the trigger-time parameter, exactly as
  before; values the control takes *within* the extent ship as
  `VoiceParamBreakpoint { position, value }` automation, where `position` is
  normalized (0..1) musical time within the extent so the data survives tempo
  changes between query and trigger. Constant-per-note controls ship nothing
  and behave bit-identically to ADR 0010.
- **How they travel.** `SampleEvent` and `SampleTrigger` carry
  `[Option<Arc<[VoiceParamBreakpoint]>>; 4]`. `Arc`-sharing keeps the
  query-side clone-heavy transforms cheap and, crucially, keeps the audio
  thread's cycle-boundary trigger clones allocation-free.
- **The cap.** `MAX_VOICE_PARAM_BREAKPOINTS = 32` per note per parameter.
  Both the query side and trigger-time stamping truncate: only the FIRST 32
  breakpoints ship; the last kept value holds for the rest of the note. The
  cap bounds `VoiceParamRamps`, the `Copy` fixed-array structure (~1 KiB)
  stamped into `GraphVoiceNote` at trigger time by `graph_note_voice_ramps`
  (positions -> frame offsets within the note's gate).
- **Playback.** `GraphVoiceBank::render_frame` evaluates each automated
  parameter every frame with **linear interpolation between breakpoints**
  (amortized O(1) via a per-note cursor), reaching each breakpoint's value
  exactly at its frame and holding the last value through the release tail.
  Interpolating rather than stepping is the click-free guarantee: the
  parameter signal is continuous piecewise-linear, so there is no
  discontinuity at a breakpoint beyond the segment slope. Notes without
  automation take the exact pre-existing constant-params path
  (bit-identical regression contract, tested).
- **Degenerate values.** Query-side validation rejects non-finite control
  values (as before). Trigger-time stamping defensively skips non-finite
  positions/values, clamps positions into [0, 1], resolves repeated frames
  last-wins, and inserts an implicit frame-0 breakpoint holding the
  trigger-time value when a ramp starts mid-note (so playback always starts
  from the note-start value).

## Consequences

- `notes |> p1(segment(8, rand) |> range(200, 2000))` now animates a filter
  WITHIN each held note; no new syntax was added.
- Sub-note `p1..p4` structure no longer retriggers notes — an intentional
  semantic change (the old fragmentation was an artifact, not a feature);
  per-event-constant controls are unchanged.
- Effective control resolution is the control pattern's event density up to
  32 breakpoints per note per parameter, rendered as per-frame interpolated
  values — smoother than block-rate, but not literal audio-rate pattern
  evaluation (out of scope, matching Tidal parity).
- A steal stamps the NEW note's automation and restarts it from age 0, like
  the ADR 0010 parameter fields; smooth steal-time parameter ramps remain
  #1424's territory.
- Only graph voices consume the breakpoints today; the analog/sample voice
  paths ignore them.
