# ADR 0002: Time Model And Scheduling

- Status: Accepted
- Date: 2026-03-08

## Context

Orpheus needs a time representation that is exact enough for musical subdivision,
stable under repeated arithmetic, and simple enough to reason about formally.
Using floating-point time in the pattern core would make common operations such as
triplet subdivision depend on rounding behavior, which is exactly the kind of
gremlin that sneaks into a scheduler and then refuses to die.

The first scheduler invariant is small but foundational: a time span is valid if
its start does not come after its end. Future pattern transforms will split,
shift, and query spans, so the base model must preserve that ordering under
subdivision.

## Decision

Adopt exact rational numbers as the runtime representation for pattern time and
model spans as ordered pairs of rationals:

- `Rational` stores normalized fractions with a positive denominator
- `TimeSpan` stores `start` and `end`
- `TimeSpan::new` rejects spans where `start > end`
- zero-length spans are allowed, which keeps the model compatible with
  instantaneous events and boundary markers
- the unit cycle is represented exactly as `0/1 .. 1/1`

The formal proof surface in `proofs/pattern_time.rs` records the same invariant
as `valid_span(start, end) == start <= end` and proves that splitting a valid
ordered range at a midpoint preserves validity for both resulting spans.

## Consequences

Positive:

- repeated subdivision such as thirds or tuplets stays exact in the core model
- runtime validation matches the proved invariant directly
- future scheduler proofs can build on a small, stable ordering lemma

Trade-offs:

- rational normalization adds a little arithmetic overhead compared with raw
  integers or floats
- runtime code must maintain canonical fraction form
- later scheduling semantics still need to define how endpoints are interpreted
  during playback, even though the ordering invariant is already fixed

This is acceptable because exact temporal semantics matter more than shaving a
few cycles off math that sits above the DSP hot path.
