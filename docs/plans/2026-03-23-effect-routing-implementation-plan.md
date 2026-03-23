# Effect Routing Implementation Plan

## Overview

Add global send/return effect buses to the DSP engine. Patterns can route
audio to named buses via `send("bus_name", level, pattern)`. Each bus
applies an effect chain (Phase 1: simple delay or reverb placeholder) and
mixes the processed signal back into the master output.

## Architecture

### Signal flow (per audio buffer)

```
voices ──mix_voices()──> dry master (L,R)
  │
  ├──send gain──> bus "reverb" buffer ──effect chain──> wet (L,R) ──┐
  ├──send gain──> bus "delay"  buffer ──effect chain──> wet (L,R) ──┤
  │                                                                 │
  └────── dry master + Σ wet returns ─────> output clamp ──> DAC
```

### Key constraints

- **Allocation-free** on the audio thread — bus buffers pre-allocated at
  creation, swapped at cycle boundaries via the existing pending-swap pattern.
- **Lock-free** — buses delivered via the existing `EngineCommand` ring buffer.
- **Per-trigger send levels** — `SampleTrigger` gains a `sends` field
  (`Vec<(Box<str>, f64)>`) resolved at evaluation time, not at render time.
- **Phase 1 effects** — simple stereo delay (feedback + mix) as a
  proof-of-concept. Full reverb and modular effect chains are Phase 2.

## Implementation Steps

### Step 1: DSP — Effect trait and delay effect

**Files:** `crates/orpheus-dsp/src/effect.rs` (new), `crates/orpheus-dsp/src/lib.rs`

- Define `Effect` trait:
  ```rust
  pub trait Effect: Send + std::fmt::Debug {
      /// Process a stereo buffer in-place. Length is always a multiple of 2
      /// (interleaved L,R pairs).
      fn process_stereo(&mut self, buffer: &mut [f32]);
      /// Reset internal state (e.g. on transport stop).
      fn reset(&mut self);
  }
  ```
- Implement `StereoDelay`:
  - Pre-allocated circular buffer sized from `delay_time_secs * sample_rate`.
  - Parameters: `delay_time_secs`, `feedback` (0..1), `mix` (dry/wet 0..1).
- Register module in `lib.rs`.

**Tests:** `crates/orpheus-dsp/tests/effect.rs`
- Unit-test delay with known impulse → verify echo at correct offset.
- Verify `reset()` clears buffer.

### Step 2: DSP — EffectBus struct and engine integration

**Files:** `crates/orpheus-dsp/src/engine.rs`, `crates/orpheus-dsp/src/command.rs`

- Add `EffectBus` struct:
  ```rust
  pub struct EffectBus {
      name: Box<str>,
      send_buffer: Vec<f32>,  // pre-allocated, sized to max buffer frames * 2
      effect: Box<dyn Effect>,
      return_level: f32,
  }
  ```
- Add `EngineCore` fields:
  - `effect_buses: Vec<EffectBus>`
  - `pending_effect_buses: Option<Vec<EffectBus>>`
- Add `EngineCommand::SetEffectBuses(Vec<EffectBus>)`.
- In `apply_command()`: store in `pending_effect_buses`.
- In `begin_cycle()`: swap pending buses in (like `pending_sample_bank`).
- In `stop_transport()`: call `reset()` on all bus effects.

### Step 3: DSP — Send routing in render loop

**Files:** `crates/orpheus-dsp/src/command.rs`, `crates/orpheus-dsp/src/engine.rs`

- Add `sends: Vec<(Box<str>, f64)>` field to `SampleTrigger`.
- Modify `ActiveVoice` to store its send list (copied at activation time).
- Modify `render_into_interleaved()`:
  1. Clear all bus send buffers at start of each buffer.
  2. In the per-frame loop, after `mix_voices()`, accumulate each voice's
     contribution to its send buses (voice mono × send_gain).
     **Alternative (simpler):** After the full buffer is mixed, process buses
     on the completed buffer. Since sends are per-trigger constants, we can
     accumulate during voice mixing.
  3. After the frame loop, process each bus's effect chain in-place.
  4. Mix bus returns into master output.

  Actually — since we process frame-by-frame, and effects (delay) need
  frame-by-frame processing too, we need to integrate per-frame:

  ```
  for each frame:
    clear bus accumulators
    for each voice:
      (L, R) = voice.next_stereo_frame()
      master += (L, R)
      for (bus_name, send_level) in voice.sends:
        bus.accumulate(L * send_level, R * send_level)
    for each bus:
      (wet_L, wet_R) = bus.process_one_frame(accumulated)
      master += (wet_L, wet_R) * return_level
    clamp & write master
  ```

  This requires a new `process_one_stereo_frame()` method on `Effect` (or
  we buffer the full block and process after — cleaner for block-based
  effects like FFT reverb, but delay works either way).

  **Decision:** Use block-based processing for flexibility:
  1. Frame loop: mix voices to master, accumulate sends into bus buffers.
  2. After frame loop: process each bus buffer in-place, then sum bus
     returns into master output buffer.

  This means the per-frame master output is written as dry only, then bus
  wet signals are added in a second pass over the output buffer.

**Tests:** `crates/orpheus-dsp/tests/effect_bus.rs`
- Trigger voice with send → verify output contains delayed echo.
- Multiple buses → verify independent processing.
- Bus with zero return level → verify silence from that bus.

### Step 4: DSP — SampleTrigger send field

**Files:** `crates/orpheus-dsp/src/command.rs`

- Add `sends` field with builder method `with_send(bus_name, level)`.
- Default: empty vec (no sends).

### Step 5: Language — `send` builtin

**Files:** `crates/orpheus-lang/src/value.rs`, `crates/orpheus-lang/src/builtins.rs`

- Add `sends: Vec<(Box<str>, f64)>` to `SampleEvent`.
- Add `PatternValueTransform::add_send(bus_name, level)`.
- Add `PatternRuntime::Send { bus_name, level, inner }` variant.
- Add `BuiltinKind::Send` with arity 3: `send("name", level, pattern)`.
- Register in `builtin_value()` and wire through `execute()`.
- Implement query logic: `apply_value_mutation` that calls `add_send`.

### Step 6: Language — Type environment

**Files:** `crates/orpheus-lang/src/types/env.rs`

- Add `send` to the type environment. Signature:
  `String -> Number -> Pattern<Sample> -> Pattern<Sample>`
  (or use the existing `sample_control_scheme()` pattern if compatible —
  but send takes a String first, so it needs a custom scheme).

### Step 7: Language — Session wiring

**Files:** `crates/orpheus-lang/src/session.rs`, `crates/orpheus-lang/src/export.rs`

- In `push_pattern_update()`: transfer `SampleEvent.sends` →
  `SampleTrigger.with_send()` calls.
- In `sample_trigger_from_event()` (export path): same.
- Add mechanism for session to create/update effect buses via
  `EngineCommand::SetEffectBuses`.

### Step 8: Tests — Integration

**Files:** `crates/orpheus-lang/tests/eval.rs`, `crates/orpheus-lang/tests/infer.rs`

- Eval test: `send("reverb", 0.5, seq("bd"))` produces `SampleEvent` with
  sends = `[("reverb", 0.5)]`.
- Infer test: `send` has correct type signature.
- Parser test: expression parses correctly (should work with existing
  grammar since `send` is just a function call).

### Step 9: Verification

- `cargo fmt --all`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets --all-features`

## Out of scope (Phase 1)

- Bus-to-bus routing / feedback prevention
- Multichannel surround
- Sidechain compression
- Dynamic send level via pattern (send amount as `Pattern<Number>`) —
  deferred to Phase 2, but the `PatternRuntime::SendPattern` variant can
  be added later following the `GainPattern` precedent.
- REPL bus management commands (`:bus new reverb(room=0.8)`) — Phase 2.
  Phase 1 uses programmatic bus creation from pattern evaluation.
- Full reverb algorithm — Phase 1 uses stereo delay as proof of concept.
