# Mask Design

**Date:** 2026-03-20

**Goal:** Add a structural `mask(gate, pattern)` combinator to Orpheus so musicians can carve rhythmic occupancy out of any pattern using another pattern's event presence.

## Surface Syntax

`mask` uses the same curried/builtin call surface as the other pattern combinators:

```orpheus
drums = mask(bd ~ cp ~, bd sn)
lead = sample("vox") |> mask(1 ~ 1 ~)
```

Direct-call and pipe forms are equivalent:

```orpheus
mask(bd ~ cp ~, bd sn)
bd sn |> mask(bd ~ cp ~)
```

## Semantics

`mask(gate, pattern)` keeps only the portions of `pattern` that overlap events produced by `gate`.

- The gate contributes only temporal occupancy.
- Gate values are ignored.
- Sample and number patterns are both valid gate kinds.
- Rest regions in the gate are closed.
- Adjacent gate events without a rest between them count as one continuous open region.

V1 uses fragment-level masking:

- if a source event partially overlaps an open gate span, Orpheus splits the source event,
- overlapping fragments survive,
- non-overlapping fragments are removed,
- surviving fragments preserve the original event value.

Examples:

- `mask(bd ~ cp ~, bd sn)` keeps the first quarter of `bd` and the third quarter of `sn`.
- `mask(1 ~ 1 ~, bd sn)` behaves identically, because only gate occupancy matters.

This keeps `mask` as rhythm algebra, not boolean theater.

## Runtime Model

`mask` should be implemented as temporal intersection over realized event spans.

- Add `BuiltinKind::Mask`.
- Add `PatternRuntime::Mask { gate, inner }`.
- Store the gate in a small runtime enum so sample and number gates can be queried uniformly.
- Query the gate and merge overlapping or adjacent open spans before fragmenting the source pattern.

The query algorithm is:

1. query source events over the requested span,
2. query gate events over the same span,
3. discard gate values and keep only gate spans,
4. merge touching or overlapping gate spans into continuous open regions,
5. intersect each source event with those open regions,
6. emit surviving fragments with the original source value.

## Type System

`mask` should be polymorphic in both gate and source pattern kind:

```text
mask : Pattern<a> -> Pattern<b> -> Pattern<b>
```

The gate and source pattern kinds are independent. The output type always matches the second argument.

## Validation

Acceptance for V1:

- `mask(bd ~ cp ~, bd sn)` splits source events at gate boundaries and keeps only overlaps,
- direct-call and pipe forms match,
- number-pattern gates behave the same as sample-pattern gates when occupancy matches,
- adjacent gate events merge into one continuous open span,
- parameterized bindings can build reusable masks,
- non-pattern gate arguments fail with clear diagnostics.
