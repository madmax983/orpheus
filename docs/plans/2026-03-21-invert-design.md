# Invert Voicing Design

Date: 2026-03-21

## Goal

Add a first harmonic voicing transform:

```orpheus
invert(n, pattern)
```

where `pattern` is any `Pattern<Number>` containing simultaneous note clusters, and `n` is a non-negative whole-number inversion count.

The feature should preserve the current language model:

- harmony remains plain number patterns
- simultaneous notes are represented as concurrent events with equal spans
- voicing is a transform over those events, not a new runtime harmony type

## Surface Semantics

Examples:

```orpheus
triad = chord(c4, 0 4 7)
first = invert(1, triad)
second = invert(2, triad)

line = chord(c4 e4, 0 7) |> invert(1)
```

Semantics:

- events are partitioned into clusters by exact `part` equality
- within each cluster, notes are ordered by ascending pitch
- `invert(1, cluster)` raises the lowest note by `12` semitones
- `invert(2, cluster)` repeats that move twice, re-sorting after each step
- clusters with zero or one note are unchanged

Examples:

- `[60, 64, 67]` -> `invert(1)` => `[64, 67, 72]`
- `[60, 64, 67]` -> `invert(2)` => `[67, 72, 76]`
- `[60, 67]` -> `invert(1)` => `[67, 72]`

This behavior should apply independently to each exact-span cluster in the queried pattern.

## Scope Constraints

V1 constraints:

- inversion count must be a constant non-negative whole number
- inversion count is not pattern-valued in v1
- clustering is by exact span equality only
- result events are re-emitted in ascending pitch order within each cluster
- duplicate pitches are allowed and remain distinct events

Explicitly out of scope:

- negative inversions
- overlap-based clustering
- drop voicings (`drop2`, `drop3`, etc.)
- open voicing helpers
- dedicated harmony value types

## Type Surface

`invert` should be a number-pattern transform:

```text
invert : Pattern<Number> -> Pattern<Number> -> Pattern<Number>
```

This matches the existing calling model:

- `invert(1, pattern)`
- `pattern |> invert(1)`

The first argument is typed as `Pattern<Number>` at HM level, but runtime validation should enforce that it collapses to a constant non-negative whole number.

## Runtime Design

Implementation should operate directly on queried number-pattern events.

Algorithm for a query span:

1. Query the input number pattern.
2. Group resulting events by exact `part` equality.
3. For each group:
   - copy pitch values
   - sort ascending
   - perform `n` inversion steps
   - rebuild events using the original span
4. Merge transformed groups and sort output events deterministically.

One inversion step:

1. take the current lowest pitch
2. add `12`
3. reinsert into the cluster
4. sort ascending again

This keeps the semantics crisp and compositional with `chord`, `degrees`, `transpose`, `within`, and `when`.

## Error Handling

Runtime errors should be clear and specific:

- negative inversion count rejected
- fractional inversion count rejected
- non-constant inversion count rejected
- non-number-pattern input rejected through the existing builtin extraction path

Diagnostic wording should stay consistent with the rest of the language, preferably using `number pattern` phrasing instead of `numeric pattern`.

## Testing

Required behavior coverage:

- `invert(1, chord(c4, 0 4 7))`
- `invert(2, chord(c4, 0 4 7))`
- `invert(1, chord(c4 e4, 0 7))`
- single-note cluster unchanged
- `degrees(...) |> chord(...) |> invert(...)` style harmonic composition

Required error coverage:

- negative count
- fractional count
- non-constant count
- non-numeric pattern input

JSON export coverage should pin at least one inverted chord progression fixture.

## Follow-Up

If this lands cleanly, the next voicing feature should probably be `drop(2, pattern)`, built on the same exact-span clustering model.
