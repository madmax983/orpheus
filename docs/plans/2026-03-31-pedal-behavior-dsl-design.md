# Pedal Behavior DSL Design

- Status: Proposed
- Date: 2026-03-31

## Goal

Design a future live-coding surface for pedal-style sound design that feels
musical and compositional in the moment, without pretending to be a literal PCB
simulator.

The target is a language where the default authoring mode is behavior-first and
pipe-shaped:

```text
input
  |> preamp(gain=18, model=jfet_clean)
  |> clip(model=silicon_hard, asymmetry=0.35)
  |> tone(model=mid_hump, tilt=0.2)
  |> level(-4.db)
  |> output
```

When topology stops being linear, the user should be able to drop into an
explicit but still readable `graph {}` form:

```text
graph {
  dry = input |> buffer()
  wet = input |> preamp(gain=28) |> clip(model=red_glass)
  mixed = mix(dry * 0.2, wet * 0.8)
  mixed |> tone(lowpass=4.2k) |> output
}
```

This surface is for musically plausible pedal behavior, not datasheet truth.
The language should feel more like "shape dirt and motion live" and less like
"author a tiny SPICE deck while the audience waits."

## Chosen Direction

The chosen design is:

- behavior-first primitives, not component primitives
- Unix-pipe style serial composition as the default authoring mode
- `graph {}` as a let-bound expression language for branching and reuse
- an internal signal graph IR as the canonical lowering target
- source-level diagnostics, with `:explain` as an advanced inspection surface

This means the language intentionally does **not** begin with:

- `pcb(...)`
- raw component/pin/net syntax
- branded pedal names
- literal part names as the public contract
- arbitrary recursive graph topology

That cut is deliberate. The pedal language should support tinkering and weird
topology experiments, but it should still feel natural inside a live-coding
idiom where the musician is thinking in behaviors, not board layout.

## Why Behavior-First

The low-level component form is more expressive in theory, but it is the wrong
default surface for live use.

In practice, pedal design moves between two mental modes:

- serial stage thinking:
  `input -> gain -> clip -> tone -> output`
- topology surgery:
  "split this branch, move clipping into feedback, blend some dry back in"

The language should mirror that reality. Most patches are serial. The
interesting exceptions are nonlinear topology changes. A behavior-first surface
fits both:

- serial work stays terse and musical
- unusual routing remains possible through `graph {}`

This also keeps the semantics honest. Orpheus can promise stable musical
behavior under composition much more convincingly than it can promise faithful
electronic simulation of every op-amp, diode, and capacitor edge case.

## Surface Syntax

### Default Authoring Mode

The common case is a linear chain using `|>`:

```text
input
  |> preamp(gain=16, model=opamp_tight)
  |> clip(model=silicon_hard, drive=0.7)
  |> tone(model=scooped_stack, cutoff=3.8k)
  |> output
```

This is the ergonomic center of gravity. Most live-coded pedal patches should
be representable as a straight chain.

### Nonlinear Topology

When a patch needs branching, recombination, reuse, or later explicit feedback,
the user enters `graph {}`:

```text
graph {
  dry = input |> buffer()
  driven = input |> preamp(gain=24) |> clip(model=germanium_soft)
  mixed = mix(dry * 0.15, driven * 0.85)
  mixed |> tone(model=mid_hump) |> output
}
```

`graph {}` should feel like a tiny let-bound expression language, not a
patch-cable or schematic syntax. Bindings define named signal expressions. The
last expression (or an explicit `... |> output`) is the result of the graph.

This keeps nonlinear graphs readable without infecting the linear surface with
too many split/merge combinators.

### No Inline Split/Mix Foundation

The initial surface should **not** standardize on `split(...) |> mix(...)` as
the main nonlinear mechanism. That style stays cute briefly and then turns into
symbolic soup once names, reuse, and feedback appear.

If small helper combinators are useful later, they can lower into `graph {}`,
but `graph {}` should be the honest nonlinear escape hatch.

## Primitive Vocabulary

The first public vocabulary should stay small and behavior-scoped.

Recommended v1 primitive categories:

- audio endpoints:
  - `input`
  - `output`
- serial audio behaviors:
  - `buffer`
  - `preamp`
  - `gain`
  - `clip`
  - `tone`
  - `filter`
  - `eq`
  - `level`
- character modifiers:
  - `sag`
  - `bias`
- control producers:
  - `constant`
  - `lfo`
  - `env_follow`
- graph combinators:
  - `mix`
- topology-only special case:
  - `feedback(...)`

This is enough to describe a lot of useful pedal behavior without turning the
language into:

- a modular synth workstation
- a DSP combinator tutorial
- a fake electronics simulator

Named pedal presets such as `green_drive(...)` or `wool_fuzz(...)` should stay
out of v1. They can be layered on top later once the primitive algebra feels
right.

## Model Naming Policy

Public model names should be vibe-first and behavior-scoped rather than
brand-first or part-first.

Examples:

```text
preamp(model=jfet_clean)
preamp(model=opamp_tight)
clip(model=silicon_hard)
clip(model=germanium_soft)
tone(model=mid_hump)
tone(model=scooped_stack)
sag(model=dying_battery)
```

This has several advantages:

- it avoids trademark/brand baggage in the public DSL
- it avoids implying exact hardware emulation
- it lets the implementation evolve while preserving stable musical meaning

The contract for `model=...` is "recognizable and stable sonic character," not
"faithful reenactment of one sacred datasheet under every operating condition."

Part-inspired calibration can still happen internally, but the public language
should not promise more than the runtime can honestly deliver.

## Canonical Internal Form

Both chain syntax and `graph {}` should lower to one internal signal graph IR.
That IR remains internal; users do not author it directly.

Conceptually, a simple chain such as:

```text
input
  |> preamp(model=jfet_clean)
  |> clip(model=silicon_hard)
  |> tone(model=mid_hump)
  |> output
```

lowers to an internal graph containing:

- typed signal nodes
- edges between node ports
- parameter blocks
- metadata about node behavior such as:
  - linear vs nonlinear
  - stateful vs stateless
  - audio vs control

The IR exists so the language has one honest substrate for:

- validation
- planning
- optimization
- rendering
- equivalence between surface forms

This is the same general design move as any sane algebraic system: derived
syntax on top, one canonical representation underneath.

## Diagnostics And `:explain`

The internal IR should stay hidden during normal use. Diagnostics must remain
source-level and musician-readable.

Representative errors:

- `mix(...)` expected 2 or more signals, got 1
- `feedback(...)` is required for recursive topology
- `wet` is referenced before it is bound
- `output` cannot be used in the middle of a chain
- this graph creates an illegal cycle

The compiler pipeline should prefer reporting the highest-level failure it can:

1. parse and name resolution
2. signal/behavior typing
3. topology validation
4. lowering
5. render planning

For advanced users, `:explain` should show the lowered plan in source terms
rather than leaking raw internal node gore.

Representative `:explain` output should describe:

- which branches were created
- which params became control inputs
- where smoothing was inserted
- whether the patch is strictly feed-forward
- where explicit feedback nodes appear

`:explain` is a dev-facing truth serum, not a replacement for ordinary error
messages.

## Signal Model

The first version should use only two signal kinds:

- `audio`
- `control`

This is intentionally smaller than a full channel-aware or synthesis-oriented
type lattice. The pedal language is not trying to solve stereo graph algebra,
mid/side transforms, or audio-rate modulation semantics in v1.

Mono-only is the right initial cut. Stereo can arrive later as an intentional
extension rather than contaminating every primitive from the start.

Potential future stereo-oriented extensions:

- `pan(...)`
- `split_lr(...)`
- `merge_lr(...)`
- `widener(...)`
- `dual(...)`

Those remain out of scope for the first slice.

## Modulation

Time-varying modulation should be first-class from the beginning, but it should
be constrained.

Policy:

- continuous parameters may accept literals or control expressions
- discrete parameters remain static
- control signals run at control rate in v1
- literals auto-lift to constant control signals when needed

Examples of parameters that may accept modulation:

- `gain`
- `drive`
- `cutoff`
- `bias`
- `blend`
- `sag`

Examples of parameters that should remain static in v1:

- `model`
- topology shape
- oversampling mode

Representative patch:

```text
graph {
  wobble = lfo(rate=0.8, depth=0.2)
  grit = env_follow(input, attack=8.ms, release=120.ms)

  input
    |> preamp(gain = 14 + grit * 6, model=jfet_clean)
    |> clip(drive = 0.5 + wobble * 0.15, model=silicon_hard)
    |> tone(cutoff = 2.2k + wobble * 700, model=mid_hump)
    |> output
}
```

The runtime should insert smoothing/clamping where appropriate so modulation is
musically usable and free from obvious zipper-noise disasters.

Audio-rate modulation stays out of scope for v1. The first pedal slice should
be expressive, not feral.

## Feedback

User-authored feedback should be legal only through one explicit safe construct
in v1. Arbitrary graph cycles should remain forbidden.

Representative shape:

```text
graph {
  driven = input |> preamp(gain=20) |> clip(model=silicon_hard)
  fed = feedback(driven, amount=0.25, delay=1.block, tone=dark)
  mix(driven, fed) |> output
}
```

Semantics:

- feedback requires explicit syntax
- feedback requires explicit delay
- feedback amount must live within a bounded safe domain
- optional return-path filtering/tone shaping is allowed
- accidental recursive cycles elsewhere are rejected

This preserves the good pedal behaviors:

- bloom
- filtered regeneration
- near-runaway squeal
- self-reinforcing grit

without allowing arbitrary recursive topology to destabilize the entire surface.

## Execution Model

The execution model should be deterministic and intentionally strict.

- audio flows through the lowered graph
- control signals run at control rate
- modulatable parameters may be smoothed automatically
- plain `graph {}` must remain acyclic
- recursive behavior is introduced only through explicit `feedback(...)`

This gives the pedal DSL a clear operational story without forcing users to
reason about unstable signal recursion, ad hoc block semantics, or accidental
cycles in ordinary let-bindings.

## Acceptance

The design is successful if a future implementation can provide:

- a behavior-first surface that feels natural in live coding
- clear separation between serial chains and nonlinear topology
- first-class control-rate modulation of continuous parameters
- explicit safe feedback without arbitrary recursive graphs
- a hidden internal graph IR with source-level diagnostics
- a usable `:explain` surface for advanced inspection
- mono-only semantics that do not block later stereo extension

## Non-Goals

Explicitly out of scope for the first slice:

- PCB/netlist authoring syntax
- literal component/pin connectivity as the public surface
- exact circuit simulation
- branded or trademark-heavy pedal naming
- part-number semantics as the public contract
- arbitrary graph cycles
- stereo as a first-class concern
- audio-rate modulation
- named pedal macros/presets
- exposing the internal IR directly

If the first implementation starts wanting too many of those, the slice has
escaped its cage and needs to be cut back down before it grows extra cursed
limbs.
