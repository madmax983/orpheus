# Analog Voice Wrapper Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a thin monophonic subtractive `AnalogVoice` wrapper in `orpheus-dsp` on top of the new DSP primitives.

**Architecture:** Add a Rust-only voice wrapper in `crates/orpheus-dsp/src/synth/voice.rs` with one explicit parameter struct and one fixed render path: oscillator source -> ladder filter -> soft saturation -> gain. Keep it isolated from `engine.rs`, `voice.rs`, and `orpheus-lang`.

**Tech Stack:** Rust 2024, `orpheus-dsp`, existing `synth` primitives, `cargo fmt`, `cargo clippy`, `cargo test`.

---

### Task 1: Add Red Wrapper Tests

**Files:**
- Create: `crates/orpheus-dsp/tests/analog_voice.rs`

**Step 1: Write the failing shape smoke test**

Add a test like:

```rust
#[test]
fn analog_voice_renders_finite_output_for_each_shape() {
    let mut voice = AnalogVoice::new(48_000.0);

    for shape in [OscShape::Saw, OscShape::Pulse, OscShape::Tri, OscShape::Noise] {
        let params = AnalogVoiceParams {
            osc_shape: shape,
            freq_hz: 220.0,
            pulse_width: 0.5,
            cutoff_hz: 1_200.0,
            resonance: 0.2,
            drive: 1.0,
            gain: 0.5,
        };

        for _ in 0..128 {
            assert!(voice.next_sample(&params).is_finite());
        }

        voice.reset();
    }
}
```

**Step 2: Write the failing reset determinism test**

Add a test proving:

- same params
- reset in between
- same output sequence

**Step 3: Write the failing pulse-width behavior test**

Add a small regression showing pulse output changes when `pulse_width` changes,
while the rest of the path stays identical.

**Step 4: Verify red**

Run:

- `cargo test -p orpheus-dsp --test analog_voice`

Expected: FAIL because `AnalogVoice`, `AnalogVoiceParams`, and `OscShape` do not
exist yet.

### Task 2: Add The Voice Wrapper Surface

**Files:**
- Create: `crates/orpheus-dsp/src/synth/voice.rs`
- Modify: `crates/orpheus-dsp/src/synth/mod.rs`
- Modify: `crates/orpheus-dsp/src/lib.rs`

**Step 1: Define the public wrapper types**

Add:

- `OscShape`
- `AnalogVoiceParams`
- `AnalogVoice`

Representative shape:

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
```

**Step 2: Export the wrapper cleanly**

Re-export the new wrapper types from:

- `crates/orpheus-dsp/src/synth/mod.rs`
- `crates/orpheus-dsp/src/lib.rs`

### Task 3: Implement Minimal Wrapper State And Reset

**Files:**
- Modify: `crates/orpheus-dsp/src/synth/voice.rs`

**Step 1: Add owned primitive state**

Have `AnalogVoice` own:

- `SawOsc`
- `PulseOsc`
- `TriOsc`
- `Noise`
- `LadderFilter`
- `SoftSat`

and any sample-rate bookkeeping needed.

**Step 2: Implement `new()` and `reset()`**

`reset()` should reset every owned primitive so the wrapper has one deterministic
starting point.

**Step 3: Make reset-focused tests green**

Run:

- `cargo test -p orpheus-dsp --test analog_voice reset`

Expected: PASS for the reset determinism coverage.

### Task 4: Implement The Fixed Subtractive Path

**Files:**
- Modify: `crates/orpheus-dsp/src/synth/voice.rs`

**Step 1: Implement source selection**

`AnalogVoice::next_sample(&AnalogVoiceParams)` should choose exactly one source:

- `Saw`
- `Pulse`
- `Tri`
- `Noise`

Use:

- `freq_hz` for `Saw`, `Pulse`, and `Tri`
- `pulse_width` only for `Pulse`
- `Noise` ignores pitch in practice but still accepts the same param struct

**Step 2: Implement the common processing path**

After source selection, process through:

1. `LadderFilter`
2. `SoftSat`
3. linear gain

Do not add envelopes or shape-specific secret behavior.

**Step 3: Make shape smoke tests green**

Run:

- `cargo test -p orpheus-dsp --test analog_voice analog_voice_renders_finite_output_for_each_shape -- --exact`
- `cargo test -p orpheus-dsp --test analog_voice`

Expected: PASS.

### Task 5: Add Red Integration Tests For Common Path Behavior

**Files:**
- Modify: `crates/orpheus-dsp/tests/analog_voice.rs`

**Step 1: Add a filter/sat/gain path test**

Add a test showing all shapes run through the same downstream path, for example:

- same `cutoff_hz`, `resonance`, `drive`, `gain`
- per-shape output remains finite
- output becomes quieter when `gain` is reduced

**Step 2: Add a pulse-width regression**

Pin that `OscShape::Pulse` changes output when `pulse_width` changes but the
wrapper remains deterministic after `reset()`.

**Step 3: Verify red if needed**

Run:

- `cargo test -p orpheus-dsp --test analog_voice`

If anything fails, fix only the minimal wrapper logic required.

### Task 6: Finalize Docs And Surface Hygiene

**Files:**
- Modify: `crates/orpheus-dsp/src/synth/voice.rs`
- Modify: `docs/plans/analog_modeled_synthesis.md`

**Step 1: Add short doc comments**

Document:

- `OscShape`
- `AnalogVoiceParams`
- `AnalogVoice`

with emphasis on this being a thin internal wrapper over the primitive algebra.

**Step 2: Update the analog synthesis plan note**

Add a short note near the top of `docs/plans/analog_modeled_synthesis.md`
pointing from "primitive kernel complete" to "voice wrapper is the next execution
slice."

### Task 7: Full Verification And Sludge Scan

**Step 1: Run full verification**

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-dsp --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-dsp --all-targets`

**Step 2: Scan touched areas for sludge**

Run:

- `rg -n "TODO|FIXME|Stub:" crates/orpheus-dsp/src/synth/voice.rs crates/orpheus-dsp/tests/analog_voice.rs docs/plans/2026-03-24-analog-voice-wrapper-design.md docs/plans/2026-03-24-analog-voice-wrapper-implementation-plan.md docs/plans/analog_modeled_synthesis.md`

**Step 3: Review final shape**

Confirm:

- wrapper remains monophonic
- wrapper owns only DSP state
- wrapper does not touch engine/event/language code
- wrapper is clearly composition glue over the primitive algebra

## Notes

- Do not integrate with `crates/orpheus-dsp/src/engine.rs` or `crates/orpheus-dsp/src/voice.rs` in this slice.
- Do not add envelopes, gate handling, or polyphony here.
- Do not add a language-level synth surface here.
- If the wrapper starts wanting multiple oscillators, modulation matrices, or graph semantics, stop. That is later work.
