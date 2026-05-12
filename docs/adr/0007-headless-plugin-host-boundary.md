# ADR 0007: Headless Plugin Host Boundary

## Status

Accepted

## Context

Orpheus needs language-level VST3 and AudioUnit instrument tracks without
letting third-party plugin loading details leak into the pattern evaluator or
mixer routing model. Real plugin SDK integration also has platform-specific
requirements that should not be forced onto the core scheduler before the
runtime contract is proven.

## Decision

The DSP crate owns a plugin-host boundary made from immutable descriptors,
cycle-local note events, cycle-local parameter automation lanes, and a render
processor facade. The language layer constructs `PluginPatternValue`s through
`vst(...)`, `au(...)`, `notes(...)`, and `p(...)`, then the mixer compiles them
into `TrackSource::Plugin` entries just like sample tracks compile into
`TrackSource::SamplePattern`.

Phase 1 ships a deterministic headless processor behind that facade. It is not
the final vendor binary loader; it exists to lock down the real-time contract:
plugin track processing happens through preallocated per-track state, accepts
MIDI-style note events, applies normalized automation, and routes stereo output
through the existing track, bus, and master mixer paths.

Future VST3 or AudioUnit SDK backends must implement the same facade and keep
binary discovery, instantiation, parameter lookup, and buffer allocation off the
audio process path. The audio thread may process already-prepared plugin state,
but it must not allocate memory or take locks inside the frame render call.

## Consequences

- The language and mixer can exercise plugin-hosting semantics without linking a
  platform plugin SDK yet.
- The offline and live engines share the same plugin-track routing shape.
- Tests can verify note routing, stereo output, and no process-path buffer
  growth deterministically.
- Real VST3/AU bundle loading remains a backend implementation task behind the
  descriptor and processor boundary.
