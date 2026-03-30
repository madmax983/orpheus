# Roll Design

## Goal

Add a first-class timing-density transform:

```orpheus
roll(steps, pattern)
```

`roll` retriggers exact-span event clusters across equal adjacent subspans of the original span. It is the "insistence" transform in the timing layer:

- `strum` performs one ordered pass through a simultaneous cluster
- `arp` traverses pitches over an arbitrary number of steps
- `roll` repeats the same cluster attack over multiple subdivisions

This gives Orpheus a clean way to express snare buzzes, ratchets, repeated chord stabs, and tremolo-like emphasis without smuggling pitch traversal semantics into the feature.

## Surface And Semantics

The v1 surface is:

```orpheus
roll(steps, pattern)
```

Examples:

```orpheus
buzz = roll(8, sn)
ratchet = roll(4, bd sn)
stabs = roll(6, chord(c4, 0 4 7))
```

Semantics:

- `steps` must be a constant positive whole number
- `pattern` may be either a sample pattern or a number pattern
- events are grouped into clusters by exact `part` equality
- each cluster span is partitioned into `steps` equal adjacent subspans
- every event in the cluster is copied onto every subspan
- the union of rewritten subspans exactly matches the original cluster span
- zero-width clusters are left unchanged
- `roll(1, pattern)` is identity

This yields:

- `roll(8, sn)` -> eight equal snare attacks across the original span
- `roll(4, chord(c4, 0 4 7))` -> four repeated triad attacks
- `roll(4, bd sn)` -> each exact-span source event is retriggered independently, not merged into one larger geometry mess

## Type And Runtime Model

`roll` should be polymorphic over pattern values, not harmony-only:

```text
roll : Pattern<Number> -> Pattern<a> -> Pattern<a>
```

This lets the same timing law serve drums and pitched material:

- sample patterns gain ratchets, rolls, and buzzes
- number patterns gain repeated stabs and tremolo-like harmonic insistence

The runtime should reuse the existing exact-span cluster rewrite style already used by `strum`, `arp`, `invert`, and `drop`.

Per queried cluster:

1. collect events with identical spans
2. if `steps == 1`, return the cluster unchanged
3. if span width is zero, return the cluster unchanged
4. compute `step = width / steps`
5. for each slot `i` in `0..steps`:
   - compute the slot subspan
   - copy every event in the cluster onto that subspan
   - clear `whole`
6. merge and sort the resulting events

No direction, target selection, gap control, or swing parameter belongs in v1. Those are separate semantic branches and should not be smuggled into the foundational timing law.

## Acceptance

Parser/type/eval acceptance:

- `roll(4, sn)` evaluates as `Pattern<Sample>`
- `roll(4, chord(c4, 0 4 7))` evaluates as `Pattern<Number>`
- `pattern |> roll(4)` matches direct-call behavior
- sample and number patterns are both accepted
- non-pattern values are rejected cleanly

Behavior acceptance:

- repeated clusters cover the original span exactly
- cluster size is preserved per slot
- event count scales by `steps`
- zero-width clusters are unchanged
- exact-span clustering only

Error acceptance:

- reject zero `steps`
- reject negative `steps`
- reject fractional `steps`
- reject non-constant `steps`

## Non-Goals

The first slice explicitly excludes:

- direction or traversal semantics
- top-note-only or selected-voice rolling
- overlap-based clustering
- sample-envelope shaping controls
- swing/gap/length modifiers

If `roll` cannot be described as "repeat the whole cluster attack across equal subspans," then it is not the v1 feature and should not be shipped under this name.
