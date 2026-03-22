# Strum Transform Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `strum(pattern)` as a deterministic cluster-to-sequence timing transform for `Pattern<Number>`.

**Architecture:** Reuse the exact-span clustering law already established by `chord`, `invert`, and `drop`. `strum` should remain a plain number-pattern transform that rewrites simultaneous clusters into equal adjacent subspans across the original cluster duration.

**Tech Stack:** Rust, `orpheus-lang`, Pest parser surface already in place, evaluator/runtime pattern machinery, JSON export tests, Verus spine proofs.

---

### Task 1: Write the failing strum tests

**Files:**
- Modify: `crates/orpheus-lang/tests/infer.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/src/export.rs`
- Create: `tests/fixtures/strum_progression_export.json`

**Step 1: Write the failing test**

Add coverage for:

- infer `strum(chord(c4, 0 4 7))` as `Pattern<Number>`
- infer pipe form
- eval equal-third partition for a triad over one cycle
- eval per-cluster behavior on sequential dyads
- eval single-note cluster unchanged
- eval composition with `drop(...)`
- eval composition with `invert(...)`
- reject sample-pattern input
- reject non-number-pattern input
- JSON export fixture for one strummed progression

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p orpheus-lang --test infer strum_
cargo test -p orpheus-lang --test eval strum_
cargo test -p orpheus-lang strum_export
```

Expected: FAIL because `strum` does not exist yet.

**Step 3: Write minimal implementation**

Do not touch parser syntax. `strum` is a builtin transform only.

**Step 4: Run test to verify it passes**

Re-run the focused commands above.

**Step 5: Commit**

```bash
git add crates/orpheus-lang/tests/infer.rs crates/orpheus-lang/tests/eval.rs crates/orpheus-lang/src/export.rs tests/fixtures/strum_progression_export.json
git commit -m "Add strum transform tests"
```

### Task 2: Add builtin surface and type scheme

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`

**Step 1: Write the failing test**

Use the Task 1 infer/eval failures as the red state.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p orpheus-lang --test infer strum_
```

Expected: FAIL because the builtin/type binding is missing.

**Step 3: Write minimal implementation**

Add:

- `BuiltinKind::Strum`
- builtin name/arity/dispatch wiring
- `builtin_value("strum")`
- HM registration using a unary number-pattern scheme

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p orpheus-lang --test infer strum_
```

Expected: infer tests pass or move the remaining failures into runtime behavior.

**Step 5: Commit**

```bash
git add crates/orpheus-lang/src/builtins.rs crates/orpheus-lang/src/value.rs crates/orpheus-lang/src/types/env.rs
git commit -m "Wire strum builtin surface"
```

### Task 3: Implement runtime cluster partitioning

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/value.rs`

**Step 1: Write the failing test**

Use the focused eval failures from Task 1 as the red state.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p orpheus-lang --test eval strum_
```

Expected: FAIL because runtime behavior is missing.

**Step 3: Write minimal implementation**

Implement a number-pattern runtime path that:

- groups events by exact `part` equality
- sorts each cluster ascending by pitch
- leaves cluster sizes `0` and `1` unchanged
- partitions each cluster span into equal adjacent subspans
- rewrites note spans into those subspans
- preserves event count and pitch multiset

Design guardrails:

- no overlap-based clustering
- no direction parameter
- no sample-pattern support

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p orpheus-lang --test eval strum_
```

Expected: eval tests pass.

**Step 5: Commit**

```bash
git add crates/orpheus-lang/src/builtins.rs crates/orpheus-lang/src/value.rs
git commit -m "Implement strum cluster partitioning"
```

### Task 4: Add export fixture coverage

**Files:**
- Modify: `crates/orpheus-lang/src/export.rs`
- Create: `tests/fixtures/strum_progression_export.json`

**Step 1: Write the failing test**

Add a direct JSON export regression for a strummed progression.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p orpheus-lang strum_export
```

Expected: FAIL until the fixture and output align.

**Step 3: Write minimal implementation**

Adjust only fixture/test expectations unless export plumbing truly needs work.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p orpheus-lang strum_export
```

Expected: PASS.

**Step 5: Commit**

```bash
git add crates/orpheus-lang/src/export.rs tests/fixtures/strum_progression_export.json
git commit -m "Pin strum JSON export output"
```

### Task 5: Verify and harden

**Files:**
- Modify as needed: touched `strum` files only

**Step 1: Write the failing test**

If any edge-case regression appears during verification, add the failing test first.

**Step 2: Run test to verify it fails**

Run focused commands for the discovered regression.

**Step 3: Write minimal implementation**

Fix only the discovered edge case. Keep the cluster law small and stable.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p orpheus-lang --test infer strum_
cargo test -p orpheus-lang --test eval strum_
cargo test -p orpheus-lang strum_export
cargo fmt --all
cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings
cargo test -p orpheus-lang --all-targets
```

Expected: all green.

**Step 5: Commit**

```bash
git add crates/orpheus-lang/src/builtins.rs crates/orpheus-lang/src/value.rs crates/orpheus-lang/src/types/env.rs crates/orpheus-lang/tests/infer.rs crates/orpheus-lang/tests/eval.rs crates/orpheus-lang/src/export.rs tests/fixtures/strum_progression_export.json
git commit -m "Finish strum timing transform"
```

### Task 6: Backfill the proof spine

**Files:**
- Create: `proofs/strum_partitions.rs`

**Step 1: Write the failing test**

Start from a minimal Verus model of span partitioning over one cluster.

**Step 2: Run test to verify it fails**

Run:

```bash
C:\Users\markm\verus\verus.exe proofs/strum_partitions.rs
```

Expected: FAIL until lemmas are written.

**Step 3: Write minimal implementation**

Prove the useful spine only:

- equal partition widths sum to the original width
- partition subspans are adjacent
- partition subspans do not overlap
- partition union covers the original span
- note count is preserved by the partition model

Do not try to prove every future ornament or ordering variant.

**Step 4: Run test to verify it passes**

Run:

```bash
C:\Users\markm\verus\verus.exe proofs/strum_partitions.rs
```

Expected: PASS.

**Step 5: Commit**

```bash
git add proofs/strum_partitions.rs
git commit -m "Prove strum partition invariants"
```
