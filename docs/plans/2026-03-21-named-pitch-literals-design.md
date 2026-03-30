# Named Pitch Literals Design

**Goal:** Add lowercase ASCII named pitch literals like `c4`, `fs4`, and `bf3` to Orpheus as absolute semitone-valued numeric patterns.

**Status:** Validated design for implementation.

## Overview

Named pitch literals should be syntax sugar over the pitch algebra that already exists, not a second pitch runtime. In v1, a named pitch literal evaluates to a `Pattern<Number>` atom whose value is an absolute semitone number in a MIDI-style domain. That means named pitches compose naturally with the existing language:

```orpheus
bass = c2 g2 bf2 c3
lead = c4 ef4 g4 bf4 |> fast(2)
answer = lead |> transpose(12)
```

This is intentionally small. We are not adding note output formatting, symbolic accidentals, Unicode accidentals, a dedicated `Pitch` type, or any inverse “show me the note name” API. The language already has relative pitch algebra through `degrees(...)` and `transpose(...)`; named pitch literals are the direct absolute-pitch sibling of that model.

## Surface Syntax

V1 pitch literals are lowercase ASCII only:

- note letters: `a b c d e f g`
- optional accidental suffix:
  - `s` for sharp
  - `f` for flat
- required octave integer suffix

Examples:

- `c4`
- `fs4`
- `bf3`
- `cs10`

Non-goals for v1:

- `c#4`
- `b♭3`
- `cis4`
- double accidentals
- uppercase note names

## Semantics

Pitch-class mapping is conventional:

- `c = 0`
- `d = 2`
- `e = 4`
- `f = 5`
- `g = 7`
- `a = 9`
- `b = 11`

Accidentals adjust that pitch class by:

- `s => +1`
- `f => -1`

Absolute semitone value is:

```text
12 * (octave + 1) + pitch_class + accidental
```

Representative examples:

- `c4 = 60`
- `a4 = 69`
- `bf3 = 58`
- `fs4 = 66`

This aligns with the existing `transpose(...)` behavior and future MIDI-facing work.

## Parsing and Representation

V1 should not introduce a new AST variant. The existing grammar already parses these tokens as identifiers, and that is good enough. The runtime/type layers should recognize valid pitch-literal-shaped identifiers and lower them into numeric pattern atoms.

That keeps the implementation small:

- no grammar changes
- no AST churn
- no dedicated runtime `Pitch` value

Instead, valid pitch literals are recognized in the identifier resolution path:

- inference resolves them as `Pattern<Number>`
- evaluation resolves them as `Value::NumberPattern(NumberPatternValue::constant(...))`

This also means existing export and transform behavior should work without special cases, because once resolved they are just number patterns.

## Diagnostics

Malformed pitch-like identifiers should fail clearly when they are being used as pitches. The minimum v1 diagnostic set should cover:

- missing octave after an otherwise pitch-like token such as `cf`
- unsupported accidental syntax such as `c#4` via normal parse failure
- invalid note spellings surfaced as targeted pitch-literal errors when recognition is unambiguous

The important design goal is to avoid degrading obvious pitch attempts into bland unresolved-identifier sludge when we can detect intent cheaply.

## Acceptance

Type/eval acceptance:

- `melody = c4 ef4 g4 bf4` infers and evaluates as `Pattern<Number>`
- `riff = fs4 a4 cs5 |> fast(2)` works without special cases
- `transpose(12, c4 e4 g4)` works
- named pitches and `degrees(...)` coexist in the same module

Behavior acceptance:

- `c4 => 60`
- `a4 => 69`
- `bf3 => 58`
- `fs4 => 66`

JSON/export acceptance:

- a melody written with named pitch literals exports through the existing number-pattern JSON path

## Non-Goals

Out of scope for this slice:

- note-name output formatting
- chord names
- uppercase spellings
- symbolic accidentals
- dedicated `Pitch` types
- inversion or voice-leading helpers

Those can come later if the instrument still wants them.
