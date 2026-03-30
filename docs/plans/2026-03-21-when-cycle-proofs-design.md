# When Cycle Proofs Design

**Date:** 2026-03-21

**Goal:** Add a Verus proof spine for `when(period, offset, transform)` that formalizes its cycle-selection semantics and the matching/non-matching partition of queried cycles.

## Scope

This proof slice targets the cycle router behind `when`, not arbitrary transform behavior. The runtime implementation in `query_when(...)` delegates to `query_transform_cycles(...)`, which walks the cycles touched by a query, clips the query to each cycle, and then routes each clipped cycle slice into either:

- the transformed branch when `cycle mod period == offset`, or
- the passthrough branch otherwise.

The proof should model exactly that boundary and no more. It must establish that `when` selects the correct cycle residue class and that the selected and unselected cycle slices form a clean partition of the queried cycle range.

## Proof Model

The proof file should use a simplified integer model in `proofs/when_cycles.rs`.

Core specs:

- `valid_when(period, offset)` iff `period > 0` and `0 <= offset < period`
- `cycle_in_query(query_start, query_end, cycle)` iff `query_start <= cycle < query_end`
- `matches_when(period, offset, cycle)` iff `cycle.rem_euclid(period) == offset`

Derived query subsets:

- matching cycles inside a query range
- non-matching cycles inside a query range

This is intentionally smaller than the Rust runtime. We are proving the cycle-selection law that the runtime depends on, not reproducing event evaluation or transform semantics inside Verus.

## Acceptance

The proof spine is complete for v1 when it establishes:

- valid `when` parameters imply a positive period and legal residue class
- matching is periodic with stride `period`
- adding any delta strictly between `0` and `period` to a matching cycle yields a non-matching cycle
- every cycle in a query range is either matching or non-matching
- no cycle in a query range is both matching and non-matching
- both subsets stay within the original query range

## Runtime Regression

Add one focused evaluator regression that checks a five-cycle query:

- `when(3, 1, rev)` affects cycles `1` and `4`
- cycles `0`, `2`, and `3` stay untransformed

That gives an executable guardrail alongside the proof spine without requiring any runtime refactor unless the proof exposes a mismatch.
