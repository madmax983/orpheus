# Invert Voicing Implementation Plan

Date: 2026-03-21

## Objective

Implement `invert(n, pattern)` for `Pattern<Number>` using exact-span note clustering and constant non-negative whole-number inversion counts.

## Task 1: Red Tests

Add failing tests before implementation.

Files:

- `crates/orpheus-lang/tests/infer.rs`
- `crates/orpheus-lang/tests/eval.rs`
- `crates/orpheus-lang/src/export.rs`
- `tests/fixtures/` (new JSON fixture)

Coverage:

- infer `invert(1, chord(c4, 0 4 7))` as `Pattern<Number>`
- infer pipe form `chord(c4, 0 4 7) |> invert(1)`
- eval first inversion triad
- eval second inversion triad
- eval inversion over sequential dyad clusters
- eval single-note cluster unchanged
- eval degree-derived harmonic composition
- reject negative count
- reject fractional count
- reject non-constant count
- export fixture for an inverted chord progression

Verification:

- run focused failing infer/eval/export tests and confirm they fail for missing `invert`

## Task 2: Builtin Surface and Type Scheme

Files:

- `crates/orpheus-lang/src/builtins.rs`
- `crates/orpheus-lang/src/value.rs`
- `crates/orpheus-lang/src/types/env.rs`

Work:

- add `BuiltinKind::Invert`
- expose `invert` from `builtin_value`
- register arity/name/execute handling
- add HM builtin scheme using the existing number-pattern transform shape

## Task 3: Runtime Implementation

Files:

- `crates/orpheus-lang/src/builtins.rs`
- `crates/orpheus-lang/src/value.rs`

Work:

- add inversion-count extraction helper:
  - constant
  - finite
  - whole-number
  - non-negative
- implement `NumberPatternValue::invert(count)`
- runtime should:
  - query the source number pattern
  - bucket events by exact `part`
  - invert each bucket independently
  - re-emit events in ascending pitch order per cluster

Design guardrails:

- no new harmony value type
- no overlap-based clustering
- no negative inversions

## Task 4: Export Fixture

Files:

- `crates/orpheus-lang/src/export.rs`
- `tests/fixtures/<new fixture>.json`

Work:

- add one direct JSON export regression that pins inverted chord output

## Task 5: Verification and Hardening

Run:

- `cargo test -p orpheus-lang --test infer invert_`
- `cargo test -p orpheus-lang --test eval invert_`
- `cargo test -p orpheus-lang invert_export`
- `cargo fmt --all`
- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-lang --all-targets`

Also scan touched files for:

- `TODO`
- `FIXME`
- `Stub:`

## Task 6: Proof Backfill

If the runtime slice stays clean, add:

- `proofs/invert_voicings.rs`

Proof scope:

- first inversion raises one lowest note by `12`
- repeated inversion preserves cluster size
- inversion preserves pitch multiset modulo octave shifts
- ordering after each step is ascending

This should be a spine proof for interval arithmetic, not a full runtime-event proof cathedral.
