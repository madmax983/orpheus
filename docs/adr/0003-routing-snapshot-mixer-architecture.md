# ADR 0003: Routing Snapshot Mixer Architecture

- Status: Proposed
- Date: 2026-03-23

## Context

Orpheus currently renders musical material through a relatively direct path:
compiled pattern updates flow into the DSP engine, the scheduler activates
voices, and the engine mixes those voices into stereo master output. That is
good enough for basic playback, but it leaves no explicit place for:

- stable track identity
- named buses
- shared send effects
- coherent mute/solo/level semantics
- future external routing and stem export

The narrower "effect routing" problem is real, but treating it as only a shared
reverb feature would be a design smell. Shared buses require a broader routing
model with stable mixer entities and safe topology mutation.

There is also a strong DX constraint: Orpheus must remain a live-coded
instrument, not a terminal recreation of Ableton Session View or a user-facing
patch-cable graph editor. The architecture therefore needs to preserve clear
audible causality while still supporting internal graph-based execution.

Finally, the real-time thread constraint is absolute. Routing changes must not
introduce allocation, locking, or partial-state observation during render.

## Decision

Adopt a graph-core, mixer-surface routing architecture built around immutable
render snapshots.

The model is:

- bindings remain language-layer musical definitions
- tracks become explicit stable mixer/runtime entities
- buses become explicit named shared destinations
- selectors and orbit-like groupings exist only as control-layer views over
  tracks
- the render thread consumes a validated immutable `RoutingSnapshot`

Topology is managed off the render thread. The control/session side builds and
validates the next routing plan, resolves selectors to concrete track IDs, and
enqueues a routing-swap command. The render thread adopts the next routing
snapshot only at a safe commit point, initially the cycle boundary.

Phase 1 public topology is intentionally constrained to:

- source binding -> track
- track -> master
- track -> bus
- bus -> master

Phase 1 does not expose arbitrary graph programming, bus-to-bus routing, or
feedback loops.

## Consequences

Positive:

- shared effects sit on a coherent mixer foundation instead of an ad hoc bus
  patch
- track identity survives binding edits and renames
- render-side routing stays deterministic and RT-safe
- the same architecture can support future TUI mixer panes, stem export, and
  external routing features
- grouping/orbit semantics can be added later without creating a second runtime
  ontology

Trade-offs:

- routing state becomes a new explicit subsystem rather than an incidental
  detail of pattern playback
- control-side validation and compilation get more complex
- phase 1 intentionally restricts topology to preserve DX clarity and RT safety
- some desired future features such as bus-to-bus processing and sidechain
  routing are deferred

This is acceptable because Orpheus needs a stable mixer spine more than it
needs immediate graph freedom. Internal graph flexibility is preserved, while
the performer-facing surface remains legible and live-coding-first.
