# ADR 0005: Microtonal and Alternate Tuning Support

- Status: Accepted
- Date: 2026-04-24

## Context

Orpheus hardcodes 12-tone equal temperament in two places:

1. `crates/orpheus-lang/src/value.rs` — `semitones_to_rate_multiplier(s) = 2^(s/12)` is the single conversion from a semitone pitch offset to the `rate` multiplier read by the analog voice path. It is invoked from `PatternRuntime::Pitch` and from the `ControlPatternKind::Pitch` control-pattern branch.
2. `crates/orpheus-dsp/src/voice.rs` — `ANALOG_BASE_FREQUENCY_HZ = 220.0` is the hardcoded reference pitch that `analog_frequency_hz` multiplies by `rate`.

This excludes global microtonal and non-Western tuning traditions (just intonation, Pythagorean, Turkish makam, gamelan, Bohlen-Pierce, Scala `.scl` libraries) and prevents users from retuning to A4 = 432 Hz or similar reference frequencies. The feature spec is in `docs/plans/microtonal_tuning_support.md`.

## Decision

Add an opt-in pattern-level tuning abstraction and a session-level reference-frequency control. Specifically:

1. Introduce `TuningValue { name, ratios: Arc<[f64]>, period: f64, ref_semitone: i32 }` as a first-class `Value` variant in `orpheus-lang`.
2. Expose three DSL builtins: `tuning(<ratios>)`, `load_scl("path")`, and `tune(t, pattern)` (pipeline form `|> tune(t)`).
3. When `tune(...)` is applied, rewrite any nested `PatternRuntime::Pitch` / `PatternRuntime::PitchPattern` subtrees into new `TunedPitch` / `TunedPitchPattern` variants that consult the tuning table instead of `semitones_to_rate_multiplier`. Patterns without `|> tune` keep 12-TET behavior bit-identically.
4. Add `EngineCommand::SetReferenceFrequency(f32)` and an engine-state `base_hz` field (default 220.0). Expose a REPL command `:ref_freq <hz>`.

### Key design choices

**Tree rewrite at pattern construction, not audio-thread lookup.**
An alternative considered was a thread-local "current tuning" or a `SetTuning` engine command that reroutes all voices at the audio layer. Rejected: it couples pattern semantics to transport state, requires rewriting the DSP voice path, and makes two patterns playing simultaneously under different tunings impossible. Construction-time tree rewrite keeps tuning a pattern property — zero audio-thread work, compositional, lets different tracks use different tunings.

**Ratios, not cents.**
`.scl` files often carry cents; we convert once at load time. Storing ratios avoids a per-event `exp2`. The existing `semitones_to_rate_multiplier` call sites do exactly one multiply per event today; the tuned variants match that cost.

**`period` field stored but validated == 2.0 in Phase 1.**
Bohlen-Pierce uses a 3/1 tritave instead of a 2/1 octave. The wrap formula `ratios[idx] * period^octave` supports any period for free. Validating `period == 2.0` in the parser is one line to drop when we expand scope — zero migration cost.

**Reference frequency is separate from tuning.**
`.scl` files are tuning-only; the companion `.kbm` (keyboard map, carrying reference pitch) is out of Phase 1 scope. Decoupling `:ref_freq` from tuning matches user intent (switch to A=432 without editing each tuning) and is a single-field engine-state change with a lock-free command.

**`ANALOG_BASE_FREQUENCY_HZ` remains the default.**
The const is renamed to `DEFAULT_ANALOG_BASE_FREQUENCY_HZ` and exported; engine state defaults to it. All existing analog-voice tests continue to pass unchanged.

**Out-of-bounds wrapping via `div_euclid` / `rem_euclid`.**
```rust
let step = (semitones.round() as i32) - ref_semitone;
let octave = step.div_euclid(n);
let idx    = step.rem_euclid(n) as usize;
ratios[idx] * period.powi(octave)
```
Handles negative semitones correctly without branches. Fractional semitones round to nearest step.

## Consequences

- `Value` gains a new `Tuning` variant; ~10 exhaustive `match` arms across `value.rs` / `builtins.rs` / `eval.rs` / `types.rs` must add a case.
- `PatternRuntime` gains `TunedPitch` / `TunedPitchPattern` variants; their Arc<TuningTable> contents keep the enum size bounded (pointer-sized).
- `gcode_export.rs` (the G-code / CNC music export) keeps its 12-TET formula unchanged. Tuning is an **audio-rendering** feature in Phase 1.
- Phase 2 candidates: non-octave periods, `.kbm` loader, MTS sysex output, dynamic Hermode-style tuning.
