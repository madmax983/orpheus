# Shared Reverb Bus Design

- Status: Proposed
- Date: 2026-03-23

## Goal

Add the second real shared effect host on top of the routing/mixer spine:
a shared bus-local reverb.

This slice should prove that Orpheus now has a real FX algebra rather than a
single special-case delay host:

- shared spatial processing on named buses
- the same immutable snapshot / mutable runtime split as delay
- cycle-safe reverb configuration swaps
- REPL and TUI visibility into hosted spatial buses

It should not widen the routing topology or turn the bus host into an effect
rack.

## Why Reverb Next

Delay already proved the hosted bus model:

- per-bus mutable state on the render thread
- wet-return mixing
- cycle-boundary configuration adoption
- state preservation across unrelated routing swaps

Reverb is the next honest operator because it occupies a different musical role:

- delay creates repeated temporal structure
- reverb creates shared diffuse space

If reverb follows the same host law as delay, the system starts to feel
algebraic rather than ad hoc. If reverb arrives through a separate surface or
runtime contract, the FX layer immediately starts lying about itself.

## Scope

Phase 1 shared-reverb scope:

- one effect per bus
- effect kind: `reverb`
- bus creation and effect hosting remain separate operations
- scalar parameters:
  - `size`
  - `damp`
  - `wet`
- all parameters are finite values in `[0, 1]`
- configuration changes adopt only at cycle boundaries
- unchanged reverb state survives unrelated routing snapshot swaps
- hosted reverb remains wet-return only

Explicitly out of scope:

- pre-delay
- modulation
- EQ/tone shaping beyond `damp`
- chained bus inserts
- bus-to-bus effect routing
- pattern-valued effect parameters
- named room/hall/plate presets as first-class semantic objects

## User Surface

The performer-facing surface should reuse the existing mixer control plane:

```text
:bus new verb
:bus fx verb reverb size=0.75 damp=0.35 wet=1.0
:send drums verb 0.20
:send pad verb 0.45
```

Clearing the host remains explicit:

```text
:bus fx verb none
```

This keeps the public model uniform:

- `:bus new` creates a stable mixer destination
- `:bus fx` selects the hosted bus-local operator
- `:send` feeds shared signal into the hosted space

The summary surface should show the host compactly, for example:

```text
bus verb -> master reverb(size=0.75 damp=0.35 wet=1.00)
```

That gives the performer one obvious answer to “why does this sound spacious?”

## Runtime Model

The routing snapshot should grow the second hosted effect descriptor:

```rust
pub enum BusEffectSpec {
    Delay(DelaySpec),
    Reverb(ReverbSpec),
}

pub struct ReverbSpec {
    size: f32,
    damp: f32,
    wet: f32,
}
```

The render engine should own the mutable runtime state:

```rust
enum BusEffectState {
    Delay(DelayState),
    Reverb(ReverbState),
}
```

That keeps the same clean separation already established by delay:

- snapshot: validated configuration only
- render engine: mutable DSP state only

This is the most important invariant in the slice. Reverb should feel like the
same hosted-bus algebra, not like a cousin that came from a different codebase.

## DSP Shape

The first reverb should use a small deterministic Schroeder / Freeverb-adjacent
topology:

- a handful of parallel feedback comb stages for decay body
- a handful of serial allpass stages for diffusion
- slightly different delay lengths per stereo side for decorrelation

Why this shape:

- cheap enough for live use
- deterministic enough for tests
- convincing enough as shared space
- does not require IR assets or file management

`size` should control the effective decay / spaciousness.
`damp` should control high-frequency loss in the feedback path.
`wet` should control return level.

This is intentionally plainspoken and not preset-driven. The goal is not to
ship “plate theology”; it is to add a shared spatial operator that obeys the
same law as delay.

## State Preservation

Unchanged reverb state must survive unrelated routing swaps.

Otherwise, the moment a performer tweaks a send level or rebinds a track, the
entire spatial tail collapses. That would make the hosted-bus model feel
cursed.

So the preservation rule should match delay exactly:

- preserve runtime state when:
  - the bus name still exists
  - the effect kind is still `reverb`
  - the reverb config is unchanged
- reset runtime state when:
  - the bus host is cleared
  - the effect kind changes
  - any reverb parameter changes
  - the bus disappears

Stable matching should continue to use control-plane bus identity, which in this
slice means bus name.

## Audio Flow

The render flow remains the same as the delay host:

1. Render voices into per-track accumulators.
2. Route dry track signal to master and any bus send accumulators.
3. For each bus:
   - take the dry bus input for the frame
   - run the hosted effect, if present
   - produce a wet return
4. Mix the wet return into master.
5. Clamp only at final master output.

Reverb remains wet-return only in v1. The dry path is still explained by track
routing, not by a second bus law.

## Validation And Errors

The control plane should reject:

- unknown bus names in `:bus fx`
- unsupported effect kinds
- missing `size`, `damp`, or `wet`
- parameters outside `[0, 1]`
- non-finite parameters

The render thread should not coerce or sanitize any of this. Bad state belongs
in the REPL/session diagnostics, not in realtime DSP.

## Testing Strategy

The first reverb host should be validated by behavior, not romance.

Required tests:

- routing snapshot accepts / rejects reverb specs correctly
- engine test: reverb bus produces a nonzero decaying tail from an impulse-like input
- engine test: dry routing remains unchanged when no reverb host is present
- engine test: reverb config changes adopt only at cycle boundaries
- engine test: unchanged reverb state survives unrelated routing snapshot swaps
- offline/live parity test for shared reverb
- session tests for:
  - `:bus fx <bus> reverb ...`
  - `:bus fx <bus> none`
- TUI / `:mixer` summary tests for hosted reverb state

Verus should stay modest again. The useful proof update is routing-topology
continuity: hosted bus effects remain annotations on bus nodes, not new edge
kinds or hidden paths.

## Recommendation

Ship one shared reverb host using the same bus-host model as delay, with a
small deterministic diffuse topology and a minimal `size` / `damp` / `wet`
surface.

That gives Orpheus the second real operator in its FX algebra:

- `delay` for repeated temporal structure
- `reverb` for shared spatial diffusion

Same snapshot law, same mixer surface, different musical operator.
