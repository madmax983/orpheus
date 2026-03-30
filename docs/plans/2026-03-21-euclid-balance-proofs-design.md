# Euclid Balance Proofs Design

**Date:** 2026-03-21

**Goal:** Add a Verus proof spine for `euclid(pulses, steps)` that formalizes its count, range, and balanced-spacing invariants without trying to verify the entire recursive Bjorklund builder.

## Scope

This proof slice targets the musical law that makes a Euclidean gate worth the name:

- the cycle is divided into exactly `steps` equal slots
- exactly `pulses` slots are open
- open slots stay inside the slot range
- inter-onset gaps are balanced, meaning each gap is either `floor(steps / pulses)` or `ceil(steps / pulses)`
- therefore adjacent gaps differ by at most `1`

The proof should not attempt to verify `build_euclid_level(...)` directly. That would be theorem bait. Instead, we prove a clean arithmetic model for a balanced Euclidean distribution and keep the runtime connection pinned with executable tests.

## Proof Model

Use a new file, `proofs/euclid_balance.rs`, with an integer slot model.

Core specs:

- `valid_euclid(pulses, steps)` iff `steps > 0` and `0 <= pulses <= steps`
- `small_gap = floor(steps / pulses)` for `pulses > 0`
- `large_gap = ceil(steps / pulses)` for `pulses > 0`
- `large_gap_count = steps mod pulses`
- `small_gap_count = pulses - large_gap_count`

Model a canonical balanced onset sequence as prefix sums over a gap sequence whose members are always either `small_gap` or `large_gap`. The proof target is the balanced law itself, not the exact runtime rotation chosen by the current recursive builder.

That last point matters. The runtime is still pinned by exact example tests like `euclid(3, 8)` and `euclid(5, 8)`, but the formal spine proves the deeper invariant: Euclidean rhythms distribute pulses as evenly as the slot count allows.

## Acceptance

The proof slice is complete for v1 when it establishes:

- valid parameters imply `steps > 0` and `0 <= pulses <= steps`
- when `pulses == 0`, there are no open slots to distribute
- when `pulses > 0`, `small_gap` and `large_gap` are positive
- `small_gap_count + large_gap_count == pulses`
- `small_gap_count * small_gap + large_gap_count * large_gap == steps`
- every modeled gap is either `small_gap` or `large_gap`
- every modeled onset slot is in `0..steps`
- onset slots are strictly increasing, so the pulse count is exact
- any two allowed gaps differ by at most `1`

## Runtime Regression

Add one focused evaluator regression for `euclid(5, 8)` that pins the current builder output as:

- open slot starts at `0/8`, `2/8`, `3/8`, `5/8`, and `6/8`

That keeps the proof honest: the formal balanced law stays abstract, while the runtime test preserves the concrete sequence the instrument currently plays.
