# Shared Delay Bus Design

- Status: Proposed
- Date: 2026-03-23

## Goal

Add the first real shared effect host on top of the new routing/mixer spine:
a tempo-locked shared delay attached to a named bus.

This slice should prove:

- stateful bus processing in the render engine
- shared send-return semantics
- cycle-safe effect configuration swaps
- TUI and REPL inspection of hosted bus effects

It should not turn routing into a second graph language or drag in a whole rack
of effects at once.

## Why Delay First

Delay is the cleanest first shared host.

It exercises the hard architectural path without the mushier validation surface
of reverb:

- a persistent per-bus processor state
- wet-return mixing
- tempo-relative timing
- audible deterministic tests from impulse-like inputs

If the first host is correct, reverb can follow later on the same bus-effect
attachment model. If the first host is sloppy, reverb will just hide the sins in
a prettier fog bank.

## Scope

Phase 1 shared-host scope:

- one effect per bus
- effect kind: `delay` only
- bus creation and effect hosting are separate operations
- delay time is tempo-locked and specified as a positive rational subdivision
- `feedback` and `wet` are scalar values in `[0, 1]`
- effect configuration changes adopt at cycle boundaries with routing snapshots
- buses remain wet-return style; no extra dry-through path is introduced by the
  host

Explicitly out of scope:

- chained bus inserts
- reverb
- bus-to-bus effect routing
- pattern-valued effect parameters
- pattern-language `send(...)` routing
- mid-cycle effect host mutation

## User Surface

The performer-facing surface should stay in the mixer control plane:

```text
:bus new dub
:bus fx dub delay time=3/16 feedback=0.45 wet=1.0
:send drums dub 0.35
:send lead dub 0.20
```

Clearing a hosted effect should also be explicit:

```text
:bus fx dub none
```

This keeps the semantics crisp:

- `:bus new` creates a stable mixer destination
- `:bus fx` configures what processor, if any, the bus hosts
- `:send` controls which tracks feed the shared host

The bus summary surface should show hosted effects in compact form, for example:

```text
drums -> groove +send dub@0.35
bus dub fx delay(time=3/16 fb=0.45 wet=1.00) -> master
```

That gives a performer one obvious answer to "where is this echo coming from?"

## Timing Semantics

Delay time in v1 is expressed as a positive rational subdivision of one cycle,
such as:

- `1/8`
- `3/16`
- `1/4`

The effect host interprets this value against the current transport cycle length
so it stays musically locked rather than using raw milliseconds.

Two deliberate rules keep the slice sane:

1. Delay host configuration changes are adopted only at cycle boundaries.
2. The delay processor itself remains stateful across ordinary render blocks.

This means bus-hosted delay behaves like a real shared effect rather than a
recreated ornament every frame.

## Runtime Model

The immutable routing snapshot should grow a bus-effect descriptor, but not the
processor state itself.

Suggested shape:

```rust
pub enum BusEffectSpec {
    Delay(DelaySpec),
}

pub struct DelaySpec {
    time: Rational,
    feedback: f32,
    wet: f32,
}
```

Then the render engine owns the mutable runtime side:

```rust
enum BusEffectState {
    Delay(DelayState),
}
```

The important split is:

- snapshot: validated configuration only
- render engine: mutable delay buffers and write heads

This matches the routing snapshot architecture already in place. The control
plane remains expressive and validating; the render thread remains frozen and
boring during the block.

## State Preservation

The first shared host needs one subtle behavior to avoid DX rot:
unchanged bus effects must preserve runtime state across ordinary routing
snapshot swaps.

Otherwise, a harmless command like rebinding a track or changing a send level
would wipe the current delay tail. That would feel haunted in the bad way.

So the preservation rule should be:

- if a bus name still exists
- and the hosted effect kind and config are unchanged
- preserve that bus effect runtime state across snapshot adoption

Reset the state only when:

- the bus effect is removed
- the effect kind changes
- the effect parameters change
- the bus itself disappears

This matching should be keyed by stable bus identity as seen by the control
plane, which in practice means bus name for this slice. Raw bus IDs are not
enough, because snapshot rebuild order can change when new buses are inserted.

## Audio Flow

The render flow for the first shared host should be:

1. Render voices into per-track accumulators.
2. Route dry track signal to master and to any bus send accumulators.
3. For each bus:
   - take the current dry bus input
   - run the hosted delay processor if present
   - produce a wet return signal
4. Mix the wet bus return into master.
5. Clamp only at final master output.

The bus host is wet-return only in v1. Track dry identity remains on the track
path. The delay bus adds shared space; it does not replace the direct signal
path or create a second dry-through law to explain.

## Validation And Errors

The bus effect surface should reject bad state early and clearly:

- unknown bus name in `:bus fx`
- unsupported effect kind
- invalid rational time syntax
- non-positive delay time
- `feedback` outside `[0, 1]`
- `wet` outside `[0, 1]`
- replacing one hosted effect with another unsupported host shape

The runtime should not try to "fix" bad values on the audio thread. All of that
belongs in the control plane before snapshot enqueue.

## Testing Strategy

The first host should be proved by deterministic behavior, not vibes.

Required tests:

- routing/builder tests for bus effect descriptors and validation
- engine tests showing a shared delay bus produces exact delayed repeats
- engine test showing ordinary dry routing remains unchanged with no hosted
  effect
- engine test showing bus effect config changes adopt at the next cycle boundary
- engine test showing unchanged hosted delay state survives unrelated routing
  snapshot swaps
- offline/live parity test for the hosted delay
- session tests for `:bus fx <bus> delay ...` and `:bus fx <bus> none`
- TUI and `:mixer` summary tests for hosted bus effects

Verus should stay modest here. The DSP internals do not need fake-formal heroics
yet. The useful proof update is to extend `proofs/routing_graph.rs` so hosted
bus effects remain annotations on bus nodes rather than introducing new allowed
edge kinds or hidden routing paths.

## Recommendation

Ship one shared delay bus host with cycle-safe adoption, state preservation for
unchanged bus configs, and summary visibility in the REPL/TUI.

That is enough to prove the architecture honestly and to give Orpheus its first
real shared-space mixing feature without turning the engine into a modular synth
tax code.
