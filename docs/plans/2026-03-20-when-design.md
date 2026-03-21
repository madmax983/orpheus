# When Design

**Date:** 2026-03-20

**Goal:** Add a cycle-conditional `when(period, offset, transform)` combinator to Orpheus so musicians can target transforms to a specific cycle offset inside a repeating block.

## Surface Syntax

`when` uses the same curried/builtin call surface as the other transform combinators:

```orpheus
drums = bd sn |> when(3, 1, rev)
lead = bd sn cp |> when(4, 2, fast(2))
```

Direct-call and pipe forms are equivalent:

```orpheus
when(3, 1, rev, bd sn)
bd sn |> when(3, 1, rev)
```

## Semantics

`when(period, offset, transform)` applies its unary transform only on cycles whose absolute index satisfies:

```text
cycle mod period == offset
```

V1 constraints:

- `period` is a constant positive whole number.
- `offset` is a constant whole number.
- `0 <= offset < period`.
- The condition is evaluated against absolute cycle numbers.
- The transform slot accepts any unary callable that maps `Pattern<t> -> Pattern<t>`.

Examples:

- `when(3, 1, rev)` transforms cycle `1`, `4`, `7`, ...
- `when(4, 0, fast(2))` is equivalent to `every(4, fast(2))`.

This keeps `when` small but not redundant: it becomes the offset-aware superset of `every` without dragging in a predicate language.

## Runtime Model

`when` should reuse the existing cycle-local transform machinery already used by `every` and `sometimes`.

- Add `BuiltinKind::When`.
- Add `PatternRuntime::When { period, offset, transform, inner }`.
- Reuse `query_transform_cycles(...)` with a different cycle predicate.
- Preserve absolute-cycle behavior for nested transforms by continuing to thread `absolute_cycle_for_runtime(...)` through localized queries.

No parser changes are needed because `when` is only a new builtin surface.

## Type System

`when` should be polymorphic over pattern kind:

```text
when : Pattern<Number> -> Pattern<Number> -> (Pattern<t> -> Pattern<t>) -> Pattern<t> -> Pattern<t>
```

Using the current representation, `period` and `offset` are still typed as numeric patterns even though runtime evaluation requires them to be constant whole numbers. This matches the existing builtin treatment used by `every`, `fast`, and friends.

## Validation

Acceptance for V1:

- parser needs no syntax changes,
- `when(3, 1, rev, bd sn)` infers `Pattern<Sample>`,
- pipe and direct-call forms match,
- transforms apply only on matching cycle offsets,
- parameterized unary transforms work inside `when`,
- invalid `period`/`offset` values fail with clear diagnostics.
