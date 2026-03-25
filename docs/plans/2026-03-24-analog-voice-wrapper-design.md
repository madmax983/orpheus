# Analog Voice Wrapper Design

- Status: Proposed
- Date: 2026-03-24

## Goal

Build the next analog synthesis slice as a thin monophonic voice wrapper in
`orpheus-dsp` that composes the new subtractive primitives without adding a
language surface, engine wiring, or graph syntax yet.

The wrapper should prove that the primitive algebra already built in
`crates/orpheus-dsp/src/synth/` is sufficient to form one coherent playable
voice path:

- source oscillator
- ladder filter
- soft saturation
- gain

without letting note scheduling, envelopes, or Orpheus syntax infect the
primitive layer.

## Why A Wrapper Now

The primitive kernel now exists:

- `SawOsc`
- `PulseOsc`
- `TriOsc`
- `Noise`
- `LadderFilter`
- `SoftSat`
- `Gain`
- `Mix`

That means the next honest question is not "more primitives or graph DSL?"
It is "can these atoms form one stable, deterministic voice?"

The wrapper is the smallest answer to that question.

It gives us:

- a proof of composition
- one place to define reset semantics across the whole path
- a reusable target for later engine integration
- a clean seam for future envelopes, note triggering, and language syntax

while still keeping the primitive layer primary.

## Scope

Phase 1 wrapper scope:

- internal Rust-only surface in `orpheus-dsp`
- monophonic only
- one fixed subtractive path
- parameter-driven rendering
- scalar, per-sample stepping
- deterministic reset behavior

Explicitly not in this slice:

- engine integration
- note-on/note-off lifecycle
- ADSR or exposed envelope policy
- polyphony
- language-level `synth(...)`
- oscillator layering or crossfading
- graph combinators

This wrapper is composition glue, not a public synthesizer theology yet.

## Voice Surface

The wrapper should expose one explicit param/state split:

```rust
pub enum OscShape {
    Saw,
    Pulse,
    Tri,
    Noise,
}

pub struct AnalogVoiceParams {
    pub osc_shape: OscShape,
    pub freq_hz: f32,
    pub pulse_width: f32,
    pub cutoff_hz: f32,
    pub resonance: f32,
    pub drive: f32,
    pub gain: f32,
}

pub struct AnalogVoice {
    // primitive state only
}
```

Why this split:

- params are the caller-provided control snapshot
- state owns the DSP memory and reset semantics
- the wrapper stays reusable from engine code later

The wrapper should not own any musical concepts beyond timbre parameters.

## Runtime Shape

`AnalogVoice` should be a thin composition of already-existing primitives:

- one `SawOsc`
- one `PulseOsc`
- one `TriOsc`
- one `Noise`
- one `LadderFilter`
- one `SoftSat`

and a cached `sample_rate_hz`.

Each oscillator can remain owned all the time even if only one is used per
sample step. That is slightly redundant, but it keeps the first wrapper simple,
deterministic, and easy to reset.

The per-sample render law should be:

1. choose one source sample based on `OscShape`
2. process it through `LadderFilter`
3. process the result through `SoftSat`
4. scale with `gain`

That is the whole wrapper.

No hidden modulation, no extra dynamics layer, no per-shape special casing
beyond source selection and pulse-width usage.

## Reset Semantics

Reset behavior matters more than it first appears.

`AnalogVoice::reset()` should:

- reset all owned oscillators
- reset the ladder filter
- reset the soft saturation stage

so replaying the same parameter stream yields the same sample stream.

That gives the wrapper a clear contract:

- same initial state
- same params
- same output

This is what later engine-triggered note starts should rely on.

## Parameter Policy

The wrapper should keep using scalar parameters only:

- `freq_hz`
- `pulse_width`
- `cutoff_hz`
- `resonance`
- `drive`
- `gain`

That mirrors the primitive design and avoids prematurely mixing the voice slice
with control-rate or pattern-rate modulation semantics.

One deliberate cut: `Noise` should ignore `freq_hz` in practice, but the wrapper
may still accept the same param struct for all shapes. Uniformity is worth more
than shape-specific API fussiness in this first pass.

## Verification Strategy

The wrapper should be test-driven with focused executable tests rather than new
proof ambitions.

Key acceptance:

- every `OscShape` renders finite output
- `reset()` restores deterministic output
- `Pulse` actually responds to `pulse_width`
- all shapes travel through the same filter/sat/gain chain
- the wrapper stays isolated from `engine.rs`, `voice.rs`, and `orpheus-lang`

Representative tests:

- shape smoke test for finite output
- deterministic reset for one repeated param stream
- pulse-width regression test
- subtractive-path integration test across multiple shapes

This slice does not need new Verus work unless a tiny helper law appears
worth proving. The value here is composition behavior, not pretending to prove
the entire voice.

## Follow-On Path

The intended order after this wrapper is:

1. integrate one thin analog voice into the existing DSP engine
2. define note-trigger/gate policy
3. only later expose a language-level synth surface

That preserves the algebraic order:

- primitives first
- wrapper second
- engine integration third
- language surface after that

If we keep that order, the eventual `synth(...)` surface will wrap a real voice
object instead of growing its own secret DSP path.
