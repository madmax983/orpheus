# Analog Engine Integration And Demo Design

- Status: Proposed
- Date: 2026-03-24

## Goal

Integrate the new analog DSP voice wrapper into the existing live engine without
inventing a parallel synth runtime, and add a showcase `.ode` patch that
demonstrates the language, mixer, and shared-FX work together in one playable
session.

The integration should preserve the current algebraic shape of Orpheus:

- musical material is still expressed as patterns
- mixer/routing stays track -> bus -> master
- synth voices are just another source traveling through the same routing and FX
  graph

This slice should make the system feel more like an instrument and less like a
bag of disconnected features.

## Chosen Surface

The first live synth path should stay inside the existing `Pattern<Sample>`
world.

User-facing source atoms:

- `saw`
- `pulse`
- `tri`
- `noise`

User-facing synth controls:

- `cutoff(...)`
- `res(...)`
- `drive(...)`
- `pw(...)`

Those compose with the existing pattern algebra:

- `fast`, `slow`, `shift`, `rev`
- `when`, `within`, `mask`, `euclid`
- `chord`, `invert`, `drop`
- `strum`, `arp`, `roll`
- mixer routing and shared delay/reverb

The point is not “sample path plus synth path.” The point is “one event algebra,
multiple render backends.”

## Architectural Cut

Routing should remain unchanged in this slice.

Tracks still compile to:

- `TrackSource::SamplePattern(...)`

That source name is slightly misleading now, but it is the right short-term
tradeoff. The trigger payload can grow without forcing a routing rewrite.

The actual architectural change belongs in the scheduler and voice/render path:

- scheduled events must carry both start time and event duration
- analog tokens use that span as their note length
- sample tokens remain one-shot playback events

So the engine evolves from “trigger-only” scheduling to “span-aware trigger”
scheduling, while still consuming the same routing snapshots and event streams.

That keeps the first synth slice honest without prematurely introducing a new
track source type or a separate synth graph surface.

## Trigger And Control Model

The current `SampleTrigger` payload should grow into a broader voice trigger for
this slice, even if the type name remains unchanged temporarily.

New per-event synth controls:

- `cutoff_hz`
- `resonance`
- `drive`
- `pulse_width`

Existing controls remain meaningful:

- `rate` -> pitch/frequency ratio for synth tokens
- `gain` -> final output gain
- `pan` -> stereo placement

Recommended defaults when controls are absent:

- cutoff: a musically useful midrange low-pass value
- resonance: modest, non-whistling default
- drive: light saturation
- pulse width: 0.5

These controls remain event-local values, not mutable global synth state. That
matters because it keeps them composable with the existing pattern machinery and
routing snapshot model.

## Render Path

Render-time dispatch should become:

1. sample-bank sample if the token resolves to real sample data
2. drum fallback for `bd`, `sn`, `cp`, `hh`
3. analog fallback for `saw`, `pulse`, `tri`, `noise`

Analog events should render through the new `AnalogVoice` wrapper with a small
click-safe edge envelope derived from the event span.

Voice recipe per token:

- `saw` -> `SawOsc -> LadderFilter -> SoftSat -> Gain`
- `pulse` -> `PulseOsc -> LadderFilter -> SoftSat -> Gain`
- `tri` -> `TriOsc -> LadderFilter -> SoftSat -> Gain`
- `noise` -> `Noise -> LadderFilter -> SoftSat -> Gain`

This slice should stay monophonic per scheduled event. Polyphony happens
because Orpheus already schedules multiple overlapping events as separate active
voices, not because one event becomes a synthesizer workstation.

## Language And Session Impact

Language/runtime work should stay narrow:

- add bare source atoms `saw`, `pulse`, `tri`, `noise`
- add builtins `cutoff`, `res`, `drive`, `pw`
- extend sample-pattern events/triggers with synth-only control fields
- keep direct-call and pipe forms working

Session/mixer impact should be intentionally tiny:

- track binding still accepts the resulting pattern as a normal routed source
- `:mixer` and TUI do not need a new “synth mode”
- shared delay/reverb buses should process synth-routed tracks the same way they
  already process sample-routed tracks

This is important: the first synth path should feel native to the mixer
architecture already built, not like a privileged special case.

## Showcase `.ode`

This slice should ship with one real showcase patch, not just tests.

The `.ode` file should demonstrate:

- sample drums
- a synth bass using `saw`
- a pulse or tri lead using `cutoff`, `res`, `drive`, `pw`
- at least one of `within`, `when`, `mask`, or `euclid`
- at least one of `strum`, `arp`, or `roll`
- a chord/voicing move

The file should be loadable in the TUI and useful as a live sandbox.

Session commands can still set up routing when performing:

- `:track new drums`
- `:track bind drums groove`
- `:bus new dub`
- `:bus fx dub delay ...`

But the `.ode` itself should already sound musically interesting before mixer
decoration, otherwise it is just a feature checklist in drag.

## Acceptance

Engine/runtime:

- synth tokens render audible non-silent output
- event spans control note duration
- synth controls flow from evaluation to rendering
- sample playback behavior does not regress
- shared delay/reverb continue to work on synth-routed tracks

Language:

- source atoms evaluate as `Pattern<Sample>`
- synth transforms compose like existing control transforms
- pipe/direct-call equivalence holds

Demo:

- one showcase `.ode` loads and plays through the TUI/session path
- it exercises both old and new features in one coherent patch

## Non-Goals

Explicitly out of scope:

- a dedicated synth track source type
- a `synth(...)` constructor surface
- envelopes as first-class language operators
- full note-type scheduling
- modulation matrices
- graph-combinator DSP syntax

If the implementation starts wanting those, it has escaped the slice and needs
to be cut back down.
