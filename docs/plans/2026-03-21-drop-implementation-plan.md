# Drop Voicing Implementation Plan

Date: 2026-03-21

## Objective

Implement `drop(k, pattern)` for `Pattern<Number>` using exact-span clustering and traditional drop-voicing semantics from the top of each cluster.

## Task 1: Red Tests

Add failing tests before implementation.

Files:

- `crates/orpheus-lang/tests/infer.rs`
- `crates/orpheus-lang/tests/eval.rs`
- `crates/orpheus-lang/src/export.rs`
- `tests/fixtures/` (new JSON fixture)

Coverage:

- infer `drop(2, chord(c4, 0 3 7 10))` as `Pattern<Number>`
- infer pipe form
- eval drop-2 voicing
- eval drop-3 voicing
- eval per-cluster behavior on sequential chord material
- eval small clusters unchanged
- eval composition with `invert` and/or `degrees`
- reject zero count
- reject negative count
- reject fractional count
- reject non-constant count
- reject non-number-pattern input
- export fixture for one dropped progression

Verification:

- run focused infer/eval/export tests and confirm they fail for missing `drop`

## Task 2: Builtin Surface and Type Scheme

Files:

- `crates/orpheus-lang/src/builtins.rs`
- `crates/orpheus-lang/src/value.rs`
- `crates/orpheus-lang/src/types/env.rs`

Work:

- add `BuiltinKind::Drop`
- expose `drop` from `builtin_value`
- register arity/name/execute handling
- add HM scheme using the existing number-pattern control shape

## Task 3: Runtime Implementation

Files:

- `crates/orpheus-lang/src/builtins.rs`
- `crates/orpheus-lang/src/value.rs`

Work:

- add count extraction helper for positive whole numbers
- implement `NumberPatternValue::drop_voice(k)`
- add runtime cluster transform using the same exact-span grouping style as `invert`

Algorithm:

- sort cluster ascending
- if cluster size < `k`, leave unchanged
- otherwise select index `len - k`
- subtract `12`
- re-sort

Design guardrails:

- no aliases like `drop2`
- no overlap-based clustering
- no harmony runtime type

## Task 4: Export Fixture

Files:

- `crates/orpheus-lang/src/export.rs`
- `tests/fixtures/<new fixture>.json`

Work:

- add one direct JSON export regression pinning dropped voicing output

## Task 5: Verification and Hardening

Run:

- `cargo test -p orpheus-lang --test infer drop_`
- `cargo test -p orpheus-lang --test eval drop_`
- `cargo test -p orpheus-lang drop_export`
- `cargo fmt --all`
- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-lang --all-targets`

Also scan touched files for:

- `TODO`
- `FIXME`
- `Stub:`

## Task 6: Proof Backfill

If the runtime slice settles cleanly, add:

- `proofs/drop_voicings.rs`

Proof scope:

- `drop(2)` lowers the second-highest note by `12`
- `drop(3)` lowers the third-highest note by `12`
- cluster size is preserved
- transposing before or after drop agrees

This should stay a spine proof for interval arithmetic, not a full event-geometry proof.
