# Within Design

**Date:** 2026-03-20

**Goal:** Add a cycle-local `within(start, end, transform)` combinator to Orpheus so users can apply existing transforms to a normalized sub-window of each cycle.

## Surface Syntax

`within` uses the ordinary curried/builtin call surface:

```orpheus
groove = bd sn cp hh |> within(0, 0.5, rev)
lead = groove |> within(0.25, 0.75, fast(2))
```

Direct-call and pipe forms are equivalent:

```orpheus
within(0, 0.5, rev, bd sn cp hh)
bd sn cp hh |> within(0, 0.5, rev)
```

## Semantics

`within(start, end, transform)` is a cycle-local window transform.

- `start` and `end` are constant numeric expressions in the closed unit interval.
- They must satisfy `0 <= start < end <= 1`.
- The window is interpreted separately for each cycle.
- Material outside the window is preserved unchanged.
- Material inside the window is localized into a temporary unit cycle, transformed, then scaled back into the original window.

This means the window behaves like a miniature cycle:

- `rev` reverses only the slice.
- `fast(2)` doubles density only inside the slice.
- `slow(2)` stretches only inside the slice.
- `shift(x)` wraps only inside the slice.

## Runtime Model

`within` should reuse the same localization strategy used by `every` and `sometimes`.

- Add `BuiltinKind::Within`.
- Add `PatternRuntime::Within { start, end, transform, inner }`.
- Store `start` and `end` as exact rationals, not floats.
- Generalize transform storage so cycle/window transforms can hold any unary callable, not just a builtin.

The localized window runtime should:

1. query the full window from the source pattern,
2. translate it to start at `0`,
3. scale it to unit width,
4. apply the transform to that unit-local runtime,
5. query the transformed runtime for the requested local slice,
6. scale and translate the result back into the original window.

## Constraints

V1 intentionally excludes:

- meter-relative `beat(...)` windows,
- dynamic/pattern-valued start and end,
- predicate-driven `when`.

## Type System

`within` should be polymorphic over pattern kind:

```text
Pattern<Number> -> Pattern<Number> -> (Pattern<a> -> Pattern<a>) -> Pattern<a> -> Pattern<a>
```

Using the existing curried representation, the practical shape is:

```text
within : Pattern<Number> -> Pattern<Number> -> (Pattern<t> -> Pattern<t>) -> Pattern<t> -> Pattern<t>
```

## Validation

Acceptance for V1:

- parser accepts direct and piped `within(...)`,
- runtime preserves outside-window material,
- runtime localizes/re-embeds transformed inside-window material correctly,
- user-defined unary transforms work inside `within`,
- invalid windows fail with clear diagnostics,
- sample and number patterns are both supported.
