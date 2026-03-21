# Euclid Design

**Date:** 2026-03-21

**Goal:** Add `euclid(pulses, steps)` to Orpheus as a reusable Euclidean rhythm gate generator that composes with `mask`.

## Surface Syntax

`euclid` is a builtin that returns a gate pattern rather than directly transforming another pattern:

```orpheus
drums = bd sn cp hh bd sn cp hh |> mask(euclid(3, 8))
hats = hh hh hh hh hh hh hh hh |> mask(euclid(5, 8))
```

Because the result is an ordinary pattern value, it can be reused:

```orpheus
clave = euclid(3, 8)
drums = bd sn cp hh bd sn cp hh |> mask(clave)
```

## Semantics

`euclid(pulses, steps)` returns a `Pattern<Number>` spanning one cycle and divided into `steps` equal slices.

- open steps are emitted as numeric events with value `1.0`
- closed steps are rests
- `steps > 0`
- `0 <= pulses <= steps`
- no rotation in v1

The generated rhythm should distribute `pulses` as evenly as possible across the cycle using Euclidean/Bjorklund-style spacing. In practice:

- `euclid(0, 8)` yields only rests
- `euclid(8, 8)` yields eight open steps
- `euclid(3, 8)` yields the familiar 3-in-8 distribution

The output is intentionally plain. `euclid` generates structure; `mask` interprets that structure as rhythmic occupancy.

## Runtime Model

`euclid` should be implemented as a builtin constructor, not a runtime transform node.

- add `BuiltinKind::Euclid`
- arity `2`
- validate constant whole-number arguments
- generate a `NumberPatternValue` from pattern nodes

Open steps become `PatternNode::atom(1.0)`. Closed steps become `PatternNode::rest()`.

No parser changes or special query behavior are needed.

## Type System

`euclid` should use the existing numeric-argument builtin style:

```text
euclid : Pattern<Number> -> Pattern<Number> -> Pattern<Number>
```

Runtime evaluation still requires constant whole-number arguments, just like other numeric builtins.

## Validation

Acceptance for v1:

- `euclid(3, 8)` produces exactly three open events across eight equal slices,
- `euclid(0, 8)` produces no events,
- `euclid(8, 8)` produces eight open events,
- `mask(euclid(3, 8), pattern)` keeps the expected slices,
- invalid pulses/steps fail clearly,
- one JSON export fixture pins a concrete `mask(euclid(...))` output.
