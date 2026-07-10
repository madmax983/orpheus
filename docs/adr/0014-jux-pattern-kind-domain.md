# 0014. `jux` Pattern-Kind Domain

Date: 2026-07-10

## Status

Accepted

## Context

`jux(f, p)` juxtaposes a pattern: it duplicates `p`, pans the original copy
hard-left and `f(p)` hard-right, and stacks the two. The pan step is essential
to the operation — without it `jux` is just an unpanned overlay. Panning is
only meaningful for events that carry a pan channel and reach an audio bus.

Orpheus has two runtime pattern kinds that carry events:

- `Value::SamplePattern` — a `PatternRuntime<SampleEvent>`. `SampleEvent` owns a
  `pan` field (and every other DSP control), and these events are what the
  engine routes to audio buses. Both drum-sample tokens (`bd sn`) and
  `voice { ... }` tokens (`acid acid`) lower to this kind: a voice binding used
  inside a pattern is resolved as a sample-like token (see the type-inference
  coercion `Type::Voice -> Pattern<Sample>` in `types/infer.rs`, and the
  evaluator's voice-token fallthrough in `eval.rs`).
- `Value::NumberPattern` — a `PatternRuntime<f64>`. Its events are bare numbers.
  Number patterns are *control signals*: `pitch(bass_deg)`, `gain(0.5 0.8)`,
  `degrees(aeolian, 0 3 7)`, and raw pitch literals (`c3 e3 g3`, which lower to
  semitone numbers) all produce number patterns. They never reach an audio bus
  on their own — they modulate a sample or voice pattern — and they carry no
  pan channel.

The original `apply_jux` accepted `SamplePattern` and rejected everything else
with `` `jux` only applies to sample patterns ``. That message was both
imprecise (voice-token patterns already work, because they *are* sample
patterns) and unhelpful (it did not say what to do with a number/pitch/degree
pattern).

## Decision

Keep `jux` restricted to audio-producing patterns that carry a pan channel —
sample patterns and voice patterns (both `Value::SamplePattern`). Do **not**
generalize it to number patterns: panning a raw control value is meaningless,
and inventing a pan channel on `f64` events would corrupt the control-pattern
semantics that the type system deliberately keeps separate
(`Pattern<Sample>`/`Pattern<Voice>` vs `Pattern<Number>`).

The only code change is the rejection message, which now names the supported
kinds and points at the fix:

> `jux` needs an audio pattern that carries a pan channel (a sample pattern or a
> voice pattern); a number, pitch, or degree pattern is a control signal with no
> pan channel, so apply `jux` to the sample or voice pattern it drives instead.

## Consequences

- Voice-token patterns keep working with `jux` (they lower to sample patterns);
  a regression test now locks that in, since it is the most common musical use.
- Number/pitch/degree patterns still error, but the error is actionable: apply
  `jux` to the sample or voice pattern the control drives (e.g.
  `acidline |> pitch(deg) |> jux(rev)`, not `deg |> jux(rev)`).
- No audio-thread code is touched; the change is confined to the non-RT
  evaluator error path.
