# Arp Transform Design

Date: 2026-03-22

## Goal

Add a repeating harmonic timing transform:

```orpheus
arp(steps, direction, pattern)
```

where:

- `steps` is a constant positive whole number
- `direction` is a first-class builtin direction value
- `pattern` is any `Pattern<Number>` containing simultaneous note clusters

This feature should extend the exact-span harmonic model already used by
`chord(...)`, `invert(...)`, `drop(...)`, and `strum(...)`.

## Surface Semantics

Examples:

```orpheus
lead = arp(8, up, chord(c4, 0 4 7))
bass = arp(6, down, chord(c3, 0 3 7 10))
```

Semantics:

- events are partitioned into clusters by exact `part` equality
- each cluster is sorted by pitch
- the original cluster span is partitioned into `steps` equal adjacent subspans
- one note is emitted per subspan
- note selection wraps cyclically if `steps > cluster_size`

For `[60, 64, 67]` over `[0, 1)`:

- `arp(5, up, ...)` => `60, 64, 67, 60, 64`
- `arp(5, down, ...)` => `67, 64, 60, 67, 64`

This makes `arp` distinct from `strum`:

- `strum` is a single ordered pass across the cluster
- `arp` is a sequencer traversal over an arbitrary number of slices

## Direction Values

V1 should expose first-class direction values:

- `up`
- `down`

These are builtin values, not strings.

This keeps the surface small and avoids stringly little goblins like
`arp(8, "up", ...)`.

Explicitly out of scope for v1:

- `updown`
- `downup`
- `random`
- user-defined direction values

## Scope Constraints

V1 constraints:

- `steps` must be constant, finite, positive, and whole-numbered
- `arp` applies to `Pattern<Number>` only
- clustering is by exact span equality only
- note ordering is pitch-based, not original insertion order
- partition subspans are equal, adjacent, and cover the original span exactly
- single-note clusters repeat the same note across all steps

Explicitly out of scope:

- sample-pattern arpeggiation
- pattern-valued step counts
- overlap-based clustering
- preserving original event insertion order
- `roll(...)`

## Type Surface

This slice needs a first-class direction type:

```text
ArpDirection
```

Surface types:

```text
up : ArpDirection
down : ArpDirection
arp : Pattern<Number> -> ArpDirection -> Pattern<Number> -> Pattern<Number>
```

Calling forms:

- `arp(8, up, chord(...))`
- `chord(...) |> arp(8, up)`

The argument order keeps pipe usage natural while preserving ordinary curried
call structure.

## Runtime Design

Implementation should reuse the existing exact-span cluster machinery, but use
an ordered traversal law instead of the one-pass `strum` law.

Per-cluster algorithm:

1. sort the cluster by pitch
2. if the cluster is empty, do nothing
3. if the cluster span has zero width, leave it unchanged
4. partition the cluster span into `steps` equal adjacent subspans
5. for each subspan index `i`, choose:
   - `up`: `i mod cluster_size`
   - `down`: `cluster_size - 1 - (i mod cluster_size)`
6. emit one event with the chosen pitch over that subspan

This preserves:

- total span coverage
- note values drawn from the original cluster
- deterministic exported ordering

## Error Handling

Runtime validation should reject:

- zero steps
- negative steps
- fractional steps
- non-constant step patterns
- non-direction second arguments
- non-number-pattern final arguments

Diagnostic wording should stay consistent with the rest of the language and
prefer `number pattern` and `arp direction` phrasing.

## Testing

Required behavior coverage:

- `arp(5, up, chord(c4, 0 4 7))`
- `arp(5, down, chord(c4, 0 4 7))`
- per-exact-span-cluster behavior on sequential harmonic material
- single-note cluster repetition
- direct-call and pipe-form equivalence
- composition with `drop(...)` and/or `invert(...)`

Required error coverage:

- zero steps
- negative steps
- fractional steps
- non-constant steps
- non-direction second argument
- non-number-pattern final argument

JSON export should pin at least one arpeggiated progression fixture, because
this feature is visible time geometry and snapshots should keep it honest.

## Follow-Up

If `arp` lands cleanly, then `roll(...)` should be the denser retriggering
cousin rather than a second arpeggiator wearing a different jacket.
