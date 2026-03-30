# Richer Effects Chain Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add insert-style `delay`, `reverb`, `chorus`, and `compressor` controls to Orpheus sample/synth patterns, including pattern-valued parameter sequencing and live/offline DSP rendering.

**Architecture:** Extend the existing sample-event control model instead of creating a second effect surface. Language builtins write insert-effect parameters onto `SampleEvent`, conversion layers forward them into `SampleTrigger`, and `ActiveVoice` hosts a fixed insert chain that processes stereo frames for both sample playback and analog voices. Keep shared bus effects untouched; this slice is per-pattern/per-voice insert processing.

**Tech Stack:** Rust 2024, existing `orpheus-lang` builtin/runtime control plumbing, existing `orpheus-dsp` live/offline render paths, deterministic unit/integration tests

---

### Task 1: Define The Public Insert Effect Surface

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/value.rs`
- Test: `crates/orpheus-lang/tests/eval.rs`

Decide and implement a narrow builtin set:
- wet/mix controls: `delay`, `reverb`, `chorus`, `compressor`
- parameter controls: `delay_time`, `delay_feedback`, `reverb_room`, `reverb_damp`, `chorus_depth`, `chorus_rate`, `compressor_threshold`, `compressor_ratio`

All controls must accept either a constant number or a number pattern and must preserve the existing split-event semantics already used by `gain`, `pan`, `rate`, and filters.

### Task 2: Thread Insert Parameters Through Runtime Values

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/export.rs`
- Modify: `crates/orpheus-lang/src/mixer.rs`
- Modify: `crates/orpheus-lang/src/session.rs`
- Test: `crates/orpheus-lang/tests/eval.rs`

Add effect fields to `SampleEvent`, extend pattern mutation/control-pattern handling, and forward the new fields anywhere a `SampleEvent` becomes a `SampleTrigger`.

### Task 3: Add DSP-Side Trigger And Voice Support

**Files:**
- Modify: `crates/orpheus-dsp/src/command.rs`
- Modify: `crates/orpheus-dsp/src/voice.rs`
- Test: `crates/orpheus-dsp/tests/offline_render.rs`
- Test: `crates/orpheus-dsp/tests/engine_commands.rs`

Extend `SampleTrigger` with insert-effect parameters, then apply the effect chain inside `ActiveVoice` after mono generation and before final stereo output leaves the voice. Keep the implementation deterministic and allocation shape consistent with the current voice/effect code.

### Task 4: Implement The Four Insert Effects

**Files:**
- Modify: `crates/orpheus-dsp/src/voice.rs`
- Optionally create: `crates/orpheus-dsp/src/effects/*.rs` if extracting helpers is cleaner
- Test: `crates/orpheus-dsp/tests/offline_render.rs`
- Test: `crates/orpheus-dsp/tests/engine_commands.rs`

Implement one minimal-but-real chain:
- chorus: short modulated stereo delay
- delay: feedback echo with tempo-relative time in cycle units
- reverb: compact diffuse tail reuse/variant of the existing deterministic topology
- compressor: broadband stereo-linked compressor with threshold/ratio and mix blending

Use sane defaults when only the main wet/mix builtin is applied.

### Task 5: Verification And Cleanup

**Files:**
- Modify as needed based on failures
- Check: `crates/orpheus-lang/src/builtins.rs`
- Check: `crates/orpheus-lang/src/value.rs`
- Check: `crates/orpheus-lang/src/export.rs`
- Check: `crates/orpheus-lang/src/mixer.rs`
- Check: `crates/orpheus-lang/src/session.rs`
- Check: `crates/orpheus-dsp/src/command.rs`
- Check: `crates/orpheus-dsp/src/voice.rs`

Run targeted language and DSP tests, then scan the affected area for `TODO`, `FIXME`, or `Stub:` markers so the feature doesn’t ship with fake bones showing through the skin.
