# Analog DSP Primitives Design

- Status: Proposed
- Date: 2026-03-24

## Goal

Build the first analog-modeled synthesis slice as a reusable DSP primitive algebra in
`orpheus-dsp`, not as a user-facing synth syntax and not as a general graph DSL.

The immediate target is a minimal subtractive kernel that can later support:

- thin playable voice wrappers
- future modulation surfaces
- later Faust-style graph composition

without forcing those concerns into the primitive layer up front.

## Why Primitives First

The primitive algebra should arrive before synth presets or graph combinators.

Why:

- reusable DSP atoms are the stable substrate
- voice wrappers are glue and can change later
- graph combinators are only honest once the primitive vocabulary is real

If Orpheus jumps straight to a user-facing synth or a graph DSL, the primitive
semantics will get entangled with:

- note triggering
- envelopes
- language syntax
- topology design
- mixer/runtime integration

That is backwards. The primitive layer should be small, compositional, and
testable enough that later higher-level surfaces feel like wrappers over truth
rather than folklore.

## Scope

Phase 1 analog DSP primitive scope:

- internal DSP-only surface in `orpheus-dsp`
- scalar, per-sample APIs
- explicit state structs
- no allocation after construction
- no locks or shared mutable state in the hot path
- no direct language exposure yet
- no pattern-rate modulation yet
- no graph DSL yet

The first kernel should be sufficient to express subtractive voices, but not yet
required to instantiate them through the Orpheus language.

## Primitive Algebra

The first algebra should be organized by semantic role:

- sources
- unary transformers
- combiners

Concretely:

- sources:
  - `SawOsc`
  - `PulseOsc`
  - `TriOsc`
  - `Noise`
- unary transformers:
  - `Gain`
  - `LadderFilter`
  - `SoftSat`
- combiners:
  - `Mix`

This is intentionally small. The goal is to get the algebra right before it gets
large.

These primitives should be reusable building blocks, not "micro-synth presets."
For example:

- bass path: `SawOsc -> LadderFilter -> SoftSat`
- lead path: `PulseOsc -> LadderFilter`
- noise texture: `Noise -> Gain -> LadderFilter`
- layered source: `Mix(SawOsc, PulseOsc) -> LadderFilter`

That is enough to prove the subtractive kernel without prematurely standardizing
envelopes, modulation, or topology syntax.

## Runtime Shape

The primitive layer should use scalar sample-stream interfaces rather than block
processors in the first slice.

Representative shapes:

```rust
pub struct SawOsc { /* phase, sample_rate, etc. */ }
pub struct LadderFilter { /* state */ }

impl SawOsc {
    pub fn reset(&mut self);
    pub fn next_sample(&mut self, freq_hz: f32) -> f32;
}

impl LadderFilter {
    pub fn reset(&mut self);
    pub fn process(&mut self, input: f32, cutoff_hz: f32, resonance: f32) -> f32;
}
```

Why scalar first:

- simplest to reason about
- easiest to test deterministically
- maps cleanly onto current per-voice stepping in the render engine
- can later be lifted into graph combinators or block processors if needed

The primitive layer should own only local DSP state. It should not know about:

- events
- notes
- patterns
- routing snapshots
- transport cycles

Those belong above it.

## First Primitive Set

### Oscillators

The oscillator core should be:

- `SawOsc` with PolyBLEP
- `PulseOsc` with PolyBLEP and pulse-width control
- `TriOsc` derived from a bandlimited square/integrator path
- `Noise` as deterministic white noise

Rationale:

- saw and pulse cover the classic subtractive core
- triangle gives a gentler source useful for bass and modulation-like timbres
- noise is necessary for hats, breath, and texture

The anti-aliasing story matters immediately. A naive oscillator core would poison
the algebra from the bottom.

### Processing

The first processing primitives should be:

- `Gain`
- `Mix`
- `LadderFilter`
- `SoftSat`

Rationale:

- gain and mix are the minimum signal-shaping glue
- ladder filter gives the subtractive kernel its character
- soft saturation gives an analog-adjacent finishing stage without requiring a
  larger nonlinear zoo

## Parameter Policy

Parameters should be plain scalar values in the first slice.

Examples:

- oscillator frequency in Hz
- pulse width
- filter cutoff in Hz
- resonance in `[0, 1]` or another explicit bounded domain
- gain amount
- saturation drive

No sample-varying control streams yet.

That cut is deliberate. It keeps the primitive algebra focused on signal
operators, while modulation remains a later integration problem between:

- DSP stepping
- note/voice semantics
- language-level control patterns

If we mix those concerns immediately, the first slice becomes harder to verify
and easier to over-design.

## Invariants

The primitive layer should carry explicit invariants where they matter.

### Signal-domain invariants

- no NaN or infinity output for sane finite inputs
- oscillator phase remains wrapped to a valid interval
- reset returns a primitive to a defined initial state
- filter state remains bounded under ordinary finite parameter ranges

### Musical acceptance

- saw and pulse should be audibly low-alias compared to naive versions
- ladder resonance should have useful character without exploding
- saturation should be monotone and usable as a finishing stage

The "musical" layer is not a proof obligation in the strict sense, but it should
still be pinned by tests and listening-oriented benchmarks.

## Verification Strategy

This slice should follow the existing SPEC-PROOF-RED-GREEN discipline.

Proof-worthy spine candidates:

- phase wrap helper laws
- bounded parameter-domain helper math
- mix/gain/saturation sanity lemmas if the supporting math is clean enough

Executable test requirements:

- happy-path unit tests for each primitive
- reset behavior tests
- edge/boundary tests for parameter extremes
- regression tests for aliasing or instability bugs
- micro-benchmarks for hot primitives if needed

One explicit cut: this slice does **not** need to prove the whole ladder filter
as a dissertation. Prove the useful helper spine where practical and keep the
runtime design honest through tests and measured behavior.

## Out Of Scope

Explicitly not in this slice:

- language-level `synth(...)` syntax
- note triggering
- ADSR or exposed envelope generators
- polyphony semantics beyond whatever later voice wrappers choose
- user-defined DSP graph syntax
- pattern-rate or sample-rate modulation surfaces
- FM, wavetable, or convolution synthesis

Those are later layers.

## Follow-On Path

The intended order after this design is:

1. implement the primitive kernel in `orpheus-dsp`
2. validate it with proofs/tests/benchmarks
3. add a thin analog voice wrapper on top
4. only after that revisit Faust-style graph combinators

That preserves the algebraic spine:

- primitives first
- voices second
- graphs third

If we keep that order, later synth and graph work can be elegant instead of
retrofit theology.
