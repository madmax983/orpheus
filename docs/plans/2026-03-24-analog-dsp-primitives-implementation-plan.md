# Analog DSP Primitives Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the first reusable analog-modeled DSP primitive algebra in `orpheus-dsp` with a minimal subtractive kernel and no user-facing synth surface yet.

**Architecture:** Add a new internal `synth` module in `orpheus-dsp` containing scalar, per-sample primitives for sources, unary transformers, and combiners. Keep the slice strictly primitive-focused: prove a small helper-math spine, add deterministic tests for reset/stability/finite output, and defer voice wrappers, envelopes, engine wiring, and graph combinators to later slices.

**Tech Stack:** Rust 2024, `orpheus-dsp`, `cargo fmt`, `cargo clippy`, `cargo test`, Verus in `proofs/analog_primitives.rs`.

---

### Task 1: Add Red Proofs And Primitive Helper Tests

**Files:**
- Create: `proofs/analog_primitives.rs`
- Create: `crates/orpheus-dsp/tests/analog_primitives.rs`

**Step 1: Write the failing proof skeleton**

Create `proofs/analog_primitives.rs` with a tiny arithmetic/model spine for:

- wrapped phase staying in `[0, 1)`
- mix/gain helper sanity
- soft saturation boundedness or monotonic helper laws, if the model stays small

Target lemmas:

- advancing a wrapped phase by a finite positive step keeps it in the valid interval
- mix endpoints return their inputs exactly at balances `0` and `1`
- soft saturation helper output remains bounded for finite input

Keep this proof file narrow. Do not fake-prove the full ladder filter or PolyBLEP internals in this batch.

**Step 2: Write failing primitive helper tests**

Add deterministic unit tests such as:

```rust
#[test]
fn mix_endpoints_return_the_original_signals() {
    assert_eq!(Mix::blend(-0.5, 0.75, 0.0), -0.5);
    assert_eq!(Mix::blend(-0.5, 0.75, 1.0), 0.75);
}

#[test]
fn gain_scales_and_reset_is_a_noop() {
    let mut gain = Gain::new();
    assert!((gain.process(0.5, 2.0) - 1.0).abs() <= f32::EPSILON);
    gain.reset();
    assert!((gain.process(0.25, 0.5) - 0.125).abs() <= f32::EPSILON);
}

#[test]
fn soft_sat_output_stays_finite_and_bounded() {
    let mut sat = SoftSat::new();
    let output = sat.process(100.0, 4.0);
    assert!(output.is_finite());
    assert!(output.abs() <= 1.0);
}
```

Also cover:

- phase accumulator wrap behavior
- soft saturation reset behavior if state exists
- helper outputs remain finite for ordinary finite inputs

**Step 3: Verify red**

Run:

- `C:\Users\markm\verus\verus.exe proofs\analog_primitives.rs`
- `cargo test -p orpheus-dsp --test analog_primitives`

Expected:

- Verus FAIL because the proof file is still stubbed
- test FAIL because the `synth` helper surface does not exist yet

### Task 2: Implement The Primitive Helper And Module Surface

**Files:**
- Create: `crates/orpheus-dsp/src/synth/mod.rs`
- Create: `crates/orpheus-dsp/src/synth/math.rs`
- Create: `crates/orpheus-dsp/src/synth/mix.rs`
- Create: `crates/orpheus-dsp/src/synth/gain.rs`
- Create: `crates/orpheus-dsp/src/synth/nonlinear.rs`
- Modify: `crates/orpheus-dsp/src/lib.rs`

**Step 1: Add the module skeleton**

Create the internal `synth` module and export the minimal primitive surface needed for tests:

- `PhaseAccumulator`
- `Mix`
- `Gain`
- `SoftSat`

Representative shape:

```rust
pub struct PhaseAccumulator { /* phase */ }

impl PhaseAccumulator {
    pub fn new() -> Self;
    pub fn reset(&mut self);
    pub fn advance(&mut self, phase_step: f32) -> f32;
    pub fn phase(&self) -> f32;
}
```

**Step 2: Implement minimal helper logic**

Add:

- wrapped phase helper logic
- scalar blend helper
- stateless gain processor or tiny resettable wrapper
- soft saturation with deterministic bounded output

Use straightforward scalar math. Do not optimize into unreadable sludge in this slice.

**Step 3: Make proof and helper tests green**

Run:

- `C:\Users\markm\verus\verus.exe proofs\analog_primitives.rs`
- `cargo test -p orpheus-dsp --test analog_primitives`

Expected: PASS.

### Task 3: Add Red Oscillator Tests

**Files:**
- Create: `crates/orpheus-dsp/tests/analog_oscillators.rs`

**Step 1: Write failing oscillator tests**

Add tests for:

```rust
#[test]
fn saw_osc_reset_is_deterministic() {
    let mut osc = SawOsc::new(48_000.0);
    let first = (0..64).map(|_| osc.next_sample(440.0)).collect::<Vec<_>>();
    osc.reset();
    let second = (0..64).map(|_| osc.next_sample(440.0)).collect::<Vec<_>>();
    assert_eq!(first, second);
}

#[test]
fn pulse_osc_outputs_remain_finite_near_the_top_of_the_band() {
    let mut osc = PulseOsc::new(48_000.0);
    for _ in 0..512 {
        assert!(osc.next_sample(10_000.0, 0.5).is_finite());
    }
}
```

Also cover:

- triangle output remains bounded
- noise source is deterministic with a fixed seed
- high-frequency PolyBLEP smoke test against a naive saw helper in the test file

The anti-alias test can be a modest smoke check such as "PolyBLEP saw has a smaller worst-case discontinuity than a naive saw at the same high frequency." Keep it deterministic and cheap.

**Step 2: Verify red**

Run:

- `cargo test -p orpheus-dsp --test analog_oscillators`

Expected: FAIL on missing oscillator primitives.

### Task 4: Implement Source Primitives

**Files:**
- Create: `crates/orpheus-dsp/src/synth/osc.rs`
- Modify: `crates/orpheus-dsp/src/synth/mod.rs`

**Step 1: Implement PolyBLEP saw and pulse**

Add:

- `SawOsc::new(sample_rate_hz: f32)`
- `SawOsc::reset()`
- `SawOsc::next_sample(freq_hz: f32) -> f32`
- `PulseOsc::new(sample_rate_hz: f32)`
- `PulseOsc::reset()`
- `PulseOsc::next_sample(freq_hz: f32, pulse_width: f32) -> f32`

Use PolyBLEP correction at discontinuities. Clamp or constrain pulse width to a sane internal range such as `(epsilon, 1 - epsilon)` to avoid pathological edges.

**Step 2: Implement triangle and noise**

Add:

- `TriOsc` derived from a bandlimited square/integrator path
- `Noise` with deterministic seeded stepping

Keep the API scalar and resettable.

**Step 3: Make oscillator tests green**

Run:

- `cargo test -p orpheus-dsp --test analog_oscillators`
- `cargo test -p orpheus-dsp --all-targets analog_osc`

Expected: PASS.

### Task 5: Add Red Ladder Filter Tests

**Files:**
- Create: `crates/orpheus-dsp/tests/analog_filter.rs`

**Step 1: Write failing filter tests**

Add tests for:

```rust
#[test]
fn ladder_filter_zero_input_stays_silent_after_reset() {
    let mut filter = LadderFilter::new(48_000.0);
    filter.reset();
    for _ in 0..256 {
        assert!(filter.process(0.0, 1_000.0, 0.2).abs() <= 1.0e-6);
    }
}

#[test]
fn ladder_filter_output_remains_finite_under_resonant_sweep() {
    let mut filter = LadderFilter::new(48_000.0);
    for step in 0..512 {
        let cutoff = 200.0 + step as f32 * 10.0;
        let output = filter.process(0.5, cutoff, 0.9);
        assert!(output.is_finite());
    }
}
```

Also cover:

- reset clears prior state
- ordinary cutoff and resonance values keep output bounded enough for continued use
- repeated processing with finite input never produces NaN/Inf

**Step 2: Verify red**

Run:

- `cargo test -p orpheus-dsp --test analog_filter`

Expected: FAIL on missing filter primitive.

### Task 6: Implement The Ladder Filter Primitive

**Files:**
- Create: `crates/orpheus-dsp/src/synth/filter.rs`
- Modify: `crates/orpheus-dsp/src/synth/mod.rs`

**Step 1: Implement a stable 4-pole ladder approximation**

Add:

- `LadderFilter::new(sample_rate_hz: f32)`
- `LadderFilter::reset()`
- `LadderFilter::process(input: f32, cutoff_hz: f32, resonance: f32) -> f32`

Keep the first implementation stable and readable. This slice is a subtractive kernel proof point, not a DSP dissertation contest.

Use sane internal clamping for cutoff and resonance to preserve stability under finite inputs.

**Step 2: Make filter tests green**

Run:

- `cargo test -p orpheus-dsp --test analog_filter`
- `cargo test -p orpheus-dsp --all-targets analog_filter`

Expected: PASS.

### Task 7: Add Red Integration Tests For Primitive Composition

**Files:**
- Modify: `crates/orpheus-dsp/tests/analog_primitives.rs`

**Step 1: Add composition tests**

Add a few deterministic integration tests over the primitive algebra itself:

```rust
#[test]
fn subtractive_chain_produces_finite_audio() {
    let mut osc = SawOsc::new(48_000.0);
    let mut filter = LadderFilter::new(48_000.0);
    let mut sat = SoftSat::new();

    for _ in 0..1024 {
        let sample = osc.next_sample(110.0);
        let filtered = filter.process(sample, 800.0, 0.4);
        let driven = sat.process(filtered, 1.5);
        assert!(driven.is_finite());
    }
}
```

Also cover:

- mixed dual-oscillator path stays finite
- reset on all primitives restores deterministic output for the same input sequence

**Step 2: Verify red**

Run:

- `cargo test -p orpheus-dsp --test analog_primitives`

Expected: FAIL until the module exports the full primitive set cleanly.

### Task 8: Make The Primitive Algebra Coherent And Green

**Files:**
- Modify: `crates/orpheus-dsp/src/synth/mod.rs`
- Modify: `crates/orpheus-dsp/src/lib.rs`
- Modify: `crates/orpheus-dsp/tests/analog_primitives.rs`

**Step 1: Finalize exports and docs**

Make the `synth` module readable and compositional:

- export the primitive types from one obvious module surface
- add short module/type docs explaining the algebraic roles
- keep `engine.rs`, `offline.rs`, `voice.rs`, and `orpheus-lang` untouched in this slice

**Step 2: Make composition tests green**

Run:

- `cargo test -p orpheus-dsp --test analog_primitives`
- `cargo test -p orpheus-dsp --all-targets`

Expected: PASS.

### Task 9: Full Verification And Sludge Scan

**Step 1: Run full verification**

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-dsp --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-dsp --all-targets`
- `C:\Users\markm\verus\verus.exe proofs\analog_primitives.rs`

**Step 2: Scan touched areas for sludge**

Run:

- `rg -n "TODO|FIXME|Stub:" proofs/analog_primitives.rs crates/orpheus-dsp/src/synth crates/orpheus-dsp/tests/analog_primitives.rs crates/orpheus-dsp/tests/analog_oscillators.rs crates/orpheus-dsp/tests/analog_filter.rs docs/plans/2026-03-24-analog-dsp-primitives-design.md docs/plans/2026-03-24-analog-dsp-primitives-implementation-plan.md`

**Step 3: Review final shape**

Confirm:

- the primitive layer remains independent from voices, events, and language syntax
- no allocation occurs in steady-state stepping
- oscillators, filter, and nonlinear stage compose cleanly
- the proof file stays honest and helper-focused instead of pretending to prove the whole synth

## Notes

- Do not touch `crates/orpheus-dsp/src/engine.rs`, `crates/orpheus-dsp/src/voice.rs`, or `crates/orpheus-lang` in this slice unless a compile fix absolutely forces it.
- Keep the first kernel subtractive and small. If the impulse to add envelopes, modulation, or graph combinators appears, that is a sign the slice is spilling into later phases.
- The intended follow-on after this plan is a thin analog voice wrapper, not a language-level graph DSL.
