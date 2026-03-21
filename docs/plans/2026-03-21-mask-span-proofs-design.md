# Mask Span Proofs Design

**Date:** 2026-03-21

**Goal:** Add a Verus proof spine for the span algebra behind `mask`, covering merged open spans and fragment boundaries without trying to formalize the entire evaluator.

## Target Surface

The proof target is the helper algebra behind runtime masking, not the whole Rust loop structure.

Relevant runtime seams:

- `merge_open_spans(...)` in `crates/orpheus-lang/src/value.rs`
- `compute_event_fragment_boundaries(...)` in `crates/orpheus-lang/src/value.rs`
- `query_mask(...)` in `crates/orpheus-lang/src/value.rs`

Those helpers determine whether masked output stays inside the source event, whether adjacent gate spans collapse into one open region, and whether non-overlapping source fragments are correctly dropped.

## Proof Model

Model spans as integer half-open intervals `[start, end)`.

Core spec surface:

- `valid_span(start, end)`
- `span_covers(start, end, slot)`
- `spans_overlap(a_start, a_end, b_start, b_end)`
- clipped overlap boundaries for a source span and gate span
- merged coverage for sorted adjacent or overlapping spans

This model intentionally ignores runtime rationals and event payloads. The proof is about span meaning, not audio samples or evaluator plumbing.

## Properties To Prove

1. Merging adjacent or overlapping gate spans preserves coverage.
2. A merged span remains valid and covers everything the original pair covered.
3. Clipped overlap boundaries always stay inside the source span.
4. If source and gate do not overlap, there is no clipped fragment to emit.
5. If a clipped fragment exists, it is valid and lies inside both the source span and the gate span.

That is enough to justify the runtime contract used by `mask`: no overlap means no output, and any emitted fragment is safely inside the open region that caused it.

## Validation

Acceptance for this proof slice:

- `proofs/mask_spans.rs` verifies with Verus
- one Rust regression test proves a source event with no gate overlap is dropped
- existing `mask` tests still pass without runtime behavior changes

This is intentionally a spine proof, not the final cathedral. It gives the rhythm algebra a formal foothold exactly where it was previously most haunted.
