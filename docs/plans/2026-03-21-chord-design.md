# Chord Design

**Goal:** Add a first harmonic layer to Orpheus with `chord(root, intervals)`, where roots are time-varying number patterns and interval sets are written as ordinary number-pattern syntax.

**Status:** Validated design for implementation.

## Overview

The first chord slice should stay brutally honest: no chord-quality theology, no voicing engine, no separate harmony runtime. Just:

```orpheus
pad = chord(c4, 0 4 7)
m7 = chord(c4, 0 3 7 10)
line = chord(c4 e4, 0 7)
```

The result is a `Pattern<Number>` with simultaneous pitch events. So:

- `chord(c4, 0 4 7)` yields `60, 64, 67`
- `chord(c4, 0 3 7 10)` yields `60, 63, 67, 70`
- `chord(c4 e4, 0 7)` yields dyads for each root event over time

This composes naturally with the pitch algebra we already have:

- named pitch literals for absolute roots
- `degrees(...)` and `pitch_class_set(...)` for relative roots
- `transpose(...)` for register shifts
- rhythm transforms like `fast`, `when`, and `within`

## Important V1 Semantic Cut

There is one necessary correction to keep the feature musical.

In Orpheus today, `0 4 7` is a sequence over time, not a simultaneous structure. If `chord(c4, 0 4 7)` were evaluated as two ordinary time-pattern arguments over the same span, it would produce a three-step melody, not a chord.

So v1 must treat the second argument as a **unit-cycle interval set**:

- it is written using ordinary number-pattern syntax
- it is queried over the unit cycle
- only the numeric values matter
- the interval event timing does not matter in v1

That means:

- `chord(c4, 0 4 7)` is a triad
- `chord(c4 e4, 0 7)` yields two dyads, not a time-fragmented hallucination

This is slightly special semantically, but it is the only reading that matches musician intent while keeping the surface syntax small.

## Type and Runtime Model

Type surface:

```text
chord : Pattern<Number> -> Pattern<Number> -> Pattern<Number>
```

The first argument is the root pattern. The second argument is the interval-set pattern.

Runtime behavior:

1. Query the root pattern normally over the requested span.
2. Query the interval pattern over the unit cycle.
3. Validate that interval values are finite numeric values.
4. For each root event, emit one event per interval value over the same span:
   - `root_value + interval_value`
5. Sort the resulting events by normal event ordering.

This keeps the implementation small and preserves the existing export and transform pipeline, because the output is still just a `Pattern<Number>`.

## Acceptance

Type/eval acceptance:

- `pad = chord(c4, 0 4 7)` infers and evaluates as `Pattern<Number>`
- `line = chord(c4 e4, 0 7)` yields dyads for each root event
- `harm = chord(degrees(aeolian, 0 2) |> transpose(60), 0 3 7)` works
- `chord(...)` composes with `fast`, `transpose`, and export

Behavior acceptance:

- `chord(c4, 0 4 7)` yields simultaneous `60, 64, 67`
- `chord(c4, 0 3 7 10)` yields `60, 63, 67, 70`
- `chord(c4 e4, 0 7)` yields `60, 67` in the first half and `64, 71` in the second half

Error acceptance:

- reject non-numeric roots
- reject non-numeric interval sets
- reject non-finite interval values

## Non-Goals

Out of scope for v1:

- named chord qualities like `major7`
- inversion helpers
- spread/drop voicings
- diatonic tertian chord builders
- interval-set deduplication
- octave normalization

Those can come later if the instrument actually wants them.
