# Strum Transform Design

Date: 2026-03-22

## Goal

Add a timing-spread transform:

```orpheus
strum(pattern)
```

where `pattern` is any `Pattern<Number>` containing simultaneous note clusters.

This feature should extend the harmonic model already established by `chord(...)`,
`invert(...)`, and `drop(...)`:

- harmony remains plain number patterns
- simultaneous notes are grouped by exact span equality
- voicing and timing transforms operate on cluster-local pitch ordering

## Surface Semantics

Examples:

```orpheus
pad = chord(c4, 0 4 7)
pluck = strum(pad)

phrase = chord(c4 e4, 0 7 10) |> drop(2) |> strum
```

Semantics:

- events are partitioned into clusters by exact `part` equality
- within each cluster, notes are ordered ascending by pitch
- a cluster of `n` notes over span `[start, end)` is rewritten into `n` adjacent
  note events whose union is exactly `[start, end)`
- each rewritten subspan has width `(end - start) / n`
- clusters of size `0` or `1` are unchanged

Example:

- `chord(c4, 0 4 7)` over `[0, 1)` becomes:
  - `60` on `[0, 1/3)`
  - `64` on `[1/3, 2/3)`
  - `67` on `[2/3, 1)`

This is intentionally deterministic. `strum` is a cluster-to-sequence transform,
not a note generator and not a direction-rich ornament system yet.

## Scope Constraints

V1 constraints:

- `strum` applies to `Pattern<Number>` only
- clustering is by exact span equality only
- ordering is ascending by pitch only
- total occupied span is preserved exactly
- note count and pitch multiset are preserved
- transformed subspans are adjacent and non-overlapping

Explicitly out of scope:

- strum direction parameters like up/down/random
- sample-pattern strumming
- onset-only spreads with overlapping durations
- dedicated harmony runtime types
- `arp(...)` and `roll(...)`

Those can come later once the timing partition law is real and tested.

## Type Surface

`strum` should use the unary number-pattern transform shape:

```text
strum : Pattern<Number> -> Pattern<Number>
```

Calling forms:

- `strum(chord(c4, 0 4 7))`
- `chord(c4, 0 4 7) |> strum`

This keeps `strum` aligned with the existing transform vocabulary and lets it
compose with `within`, `when`, `mask`, `invert`, and `drop`.

## Runtime Design

Implementation should mirror the existing exact-span cluster machinery.

Algorithm for a query span:

1. query the input number pattern
2. group events by exact `part` equality
3. for each cluster:
   - sort ascending by pitch
   - if cluster length is `0` or `1`, leave unchanged
   - otherwise partition the original span into `n` equal adjacent subspans
   - assign note `i` to subspan `i`
4. merge transformed groups and sort deterministically

For a cluster over `[start, end)` with `n` notes:

- `width = end - start`
- `step = width / n`
- note `i` receives:
  - `start_i = start + i * step`
  - `end_i = start + (i + 1) * step`

Important invariants:

- transformed notes cover exactly the original cluster span
- transformed notes do not overlap
- transformed notes stay ordered in time
- event count is unchanged

## Error Handling

Runtime validation should reject:

- sample patterns
- non-number-pattern final arguments
- non-finite numeric event values if encountered in the cluster

Diagnostic wording should stay consistent with the rest of the language and
prefer `number pattern` phrasing.

## Testing

Required behavior coverage:

- `strum(chord(c4, 0 4 7))` yields equal thirds across one cycle
- `strum(chord(c4 e4, 0 7))` strums each half-cycle dyad independently
- single-note clusters are unchanged
- composition with `drop(...)`
- composition with `invert(...)`
- pipe and direct-call forms agree

Required error coverage:

- reject sample-pattern input
- reject non-number-pattern input

JSON export coverage should pin at least one strummed progression fixture,
because this feature is exactly the sort of time geometry that snapshots should
make brutally obvious.

## Follow-Up

If `strum` lands cleanly, then:

- `arp(...)` can build on the same cluster ordering and partition law
- `roll(...)` can build on repeated retriggering over a preserved span

But those should be deliberate extensions, not co-designed in a semantic pileup.
