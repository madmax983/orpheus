# Degrees Design

**Date:** 2026-03-21

**Goal:** Add `degrees(name, pattern)` and `transpose(offset, pattern)` to Orpheus as the first pitch algebra slice, using canonical collection names and interval-first semantics.

## Surface Syntax

Pitch in v1 stays numeric and algebraic:

```orpheus
riff = degrees("aeolian", -2 0 2 4 7 8)
bass = riff |> transpose(45)
hook = degrees("minor_pentatonic", 0 1 2 4) |> transpose(60)
```

Semantics:

- `degrees(name, pattern)` interprets a number pattern as zero-based scale degrees
- `transpose(offset, pattern)` shifts a number pattern by semitone offsets
- collection names are canonical in v1: `ionian`, `dorian`, `phrygian`, `mixolydian`, `aeolian`, `minor_pentatonic`
- no note-name syntax, tonic spelling, or user-defined collections in this slice

This keeps the core model orthogonal: melodic contour first, register second.

## Degree Mapping

`degrees(name, pattern)` returns a `Pattern<Number>` of semitone offsets rooted at `0`.

For a collection with interval table `intervals` and length `n`:

- `octave = floor_div(degree, n)`
- `index = degree mod n`, always in `0..n-1`
- `result = octave * 12 + intervals[index]`

Negative degrees are legal and use Euclidean division semantics instead of truncating division. With `aeolian = [0, 2, 3, 5, 7, 8, 10]`:

- degree `-1` -> `-2`
- degree `-2` -> `-4`
- degree `0` -> `0`
- degree `7` -> `12`
- degree `8` -> `14`

`degrees` requires integer-valued degree events. Fractional degrees should fail clearly instead of pretending quarter-degrees are already a theory.

## Runtime Model

Both builtins should stay in `orpheus-lang` as number-pattern operations.

- `degrees` is a builtin with arity `2`
- the first argument is a string collection name
- the second argument is a number pattern
- `transpose` is a builtin with arity `2`
- its offset should follow the existing pattern-valued control style, so constant and pattern-valued transposition both work

Under the hood, these should become runtime nodes on `NumberPatternValue`, not eager evaluation hacks. That preserves composition under `fast`, `slow`, `shift`, `mask`, `within`, and `when`.

## Proof Spine

This is a good place to put Verus back on the leash.

Add `proofs/pitch_degrees.rs` as the proof surface for the degree-mapping math:

- canonical interval tables are ordered and remain inside one octave
- Euclidean division on signed degrees yields an index in bounds
- the computed semitone mapping matches octave-carry semantics for positive and negative degrees
- transposition preserves interval differences between events

The proof scope should stay mathematical. Refinement from runtime `f64` events into the integer degree model can remain deferred, but the algebra itself should stop living only in vibes and Rust tests.

## Type System

The builtins should infer as:

```text
degrees : String -> Pattern<Number> -> Pattern<Number>
transpose : Pattern<Number> -> Pattern<Number> -> Pattern<Number>
```

That keeps `degrees("aeolian")` reusable as a partial application and lets `transpose` support pattern-valued offsets without inventing a separate pitch type.

## Validation

Acceptance for v1:

- `degrees("aeolian", 0 2 4 7 8)` maps to `0 3 7 12 14`
- negative degrees map downward correctly
- `transpose(12, pattern)` and `transpose(12 -12, pattern)` both work
- unknown collection names fail clearly
- non-integer degree events fail clearly
- JSON export pins one parameterized melodic binding fixture
- `proofs/pitch_degrees.rs` verifies with Verus
