# Within Window Proofs Design

**Date:** 2026-03-21

**Goal:** Add a Verus proof spine for the window geometry behind `within`, covering window validity, localization, restoration, and round-trip behavior without trying to prove arbitrary transforms.

## Target Surface

The proof target is the span math behind runtime window localization, not the entire `within` evaluator.

Relevant runtime seams:

- `within_window_span(...)` in `crates/orpheus-lang/src/value.rs`
- `window_width(...)` in `crates/orpheus-lang/src/value.rs`
- `localize_span_to_window(...)` in `crates/orpheus-lang/src/value.rs`
- the span part of `restore_window_localized_events(...)` in `crates/orpheus-lang/src/value.rs`

These helpers are the real foundation of `within`: they turn a selected slice of a cycle into a temporary unit space and then restore transformed events back into the original window.

## Proof Model

Model a window as an integer half-open interval `[window_start, window_end)` with `window_start < window_end`.

Model a source span inside the window as `[span_start, span_end)` with:

- `window_start <= span_start`
- `span_start <= span_end`
- `span_end <= window_end`

Instead of reproducing runtime rationals directly, model localization as numerator space over an implicit denominator equal to the window width:

- `window_width = window_end - window_start`
- `localize_point = point - window_start`
- `restore_point = window_start + local_point`

This captures the same geometry as normalization into `[0, 1]` without dragging the full runtime arithmetic model into Verus.

## Properties To Prove

1. Valid windows have positive width.
2. Localizing an in-window span yields a valid local span with endpoints inside `[0, width]`.
3. Restoring a valid local span yields a valid in-window span.
4. Restoring a localized point or span returns the original point or span.
5. Localization preserves endpoint ordering.
6. Restoration preserves endpoint ordering.

That is enough to justify the core claim of `within`: the window behaves like a temporary local cycle, and the evaluator does not invent, lose, or invert time while entering and leaving that space.

## Validation

Acceptance for this proof slice:

- `proofs/within_windows.rs` verifies with Verus
- one Rust regression test proves `within(0, 1, transform)` matches applying the transform over the full cycle
- existing `within` eval/type tests still pass without runtime behavior changes

This is intentionally only the geometry spine. If we want more later, the next natural proof slice is the `before/window/after` partition of the queried cycle slice.
