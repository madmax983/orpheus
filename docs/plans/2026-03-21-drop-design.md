# Drop Voicing Design

Date: 2026-03-21

## Goal

Add a second voicing transform:

```orpheus
drop(k, pattern)
```

where `pattern` is any `Pattern<Number>` containing simultaneous note clusters, and `k` is a constant positive whole number interpreted from the top of each exact-span cluster.

This feature should extend the harmonic model already established by `chord(...)` and `invert(...)`:

- harmony remains plain number patterns
- simultaneous notes are grouped by exact span equality
- voicing transforms act on cluster-local pitch ordering

## Surface Semantics

Examples:

```orpheus
m7 = chord(c4, 0 3 7 10)
d2 = drop(2, m7)
d3 = drop(3, m7)

line = chord(c4 e4, 0 7 10) |> drop(2)
```

Semantics:

- events are partitioned into clusters by exact `part` equality
- within each cluster, notes are ordered ascending by pitch
- `drop(2, cluster)` lowers the second-highest note by `12` semitones
- `drop(3, cluster)` lowers the third-highest note by `12` semitones
- clusters smaller than `k` are unchanged

Examples:

- `[60, 64, 67, 70]` -> `drop(2)` => `[55, 60, 64, 70]`
- `[60, 64, 67, 70]` -> `drop(3)` => `[52, 64, 67, 70]`

This is traditional drop-voicing semantics from the top, not a generic index trick wearing harmony makeup.

## Scope Constraints

V1 constraints:

- `k` must be a constant positive whole number
- `k` is not pattern-valued in v1
- clustering is by exact span equality only
- result events are emitted in ascending pitch order within each cluster
- duplicate pitches remain distinct events
- clusters too small for the requested drop are left unchanged

Explicitly out of scope:

- `drop2` / `drop3` aliases
- dropping multiple ranked voices in one call
- overlap-based clustering
- dedicated harmony value types
- voice identity tracking across time

## Type Surface

`drop` should use the same HM shape as the other number-pattern control transforms:

```text
drop : Pattern<Number> -> Pattern<Number> -> Pattern<Number>
```

Calling forms:

- `drop(2, pattern)`
- `pattern |> drop(2)`

As with `invert`, the runtime should refine the first argument further to a constant positive whole number.

## Runtime Design

Implementation should mirror the inversion path closely.

Algorithm for a query span:

1. query the input number pattern
2. group events by exact `part` equality
3. for each cluster:
   - sort ascending by pitch
   - if `cluster.len() < k`, leave unchanged
   - otherwise lower the target note by `12`
   - re-sort ascending
4. merge transformed groups and sort deterministically

Target selection:

- if cluster length is `n`, the drop target index is `n - k`
- because the cluster is sorted ascending, this selects the `k`th note from the top

This keeps `drop` aligned with `invert` while preserving traditional voicing semantics.

## Error Handling

Runtime validation should reject:

- `drop(0, ...)`
- negative counts
- fractional counts
- non-constant counts
- non-number-pattern final arguments

Diagnostic wording should stay consistent with the rest of the language, preferring `number pattern` phrasing.

## Testing

Required behavior coverage:

- `drop(2, chord(c4, 0 3 7 10))`
- `drop(3, chord(c4, 0 3 7 10))`
- `drop(2, chord(c4 e4, 0 7 10))`
- clusters smaller than `k` remain unchanged
- composition with `degrees`, `transpose`, and `invert`

Required error coverage:

- zero count
- negative count
- fractional count
- non-constant count
- non-number-pattern input

JSON export coverage should pin at least one dropped-voicing progression fixture.

## Follow-Up

If `drop` lands cleanly, the next harmonic frontier is probably either:

- arpeggiation / strum transforms
- or named chord-quality sugar built as interval bindings, not as a new runtime type
