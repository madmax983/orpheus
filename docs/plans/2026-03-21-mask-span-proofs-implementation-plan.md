# Mask Span Proofs Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Verus proofs for the span algebra behind `mask` and tighten one runtime regression around non-overlapping fragments.

**Architecture:** Create a small proof file, `proofs/mask_spans.rs`, that models half-open interval overlap, clipping, and pairwise merging. Keep the proof mathematical and local to the helper contracts already used by `query_mask(...)`, and add one Rust regression test to pin the “no overlap means no emitted fragment” rule at the evaluator boundary.

**Tech Stack:** Rust 2024, Verus, `orpheus-lang` eval tests

---

### Task 1: Add The Red Slice

**Files:**
- Create: `proofs/mask_spans.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`

**Step 1: Add a regression test**

- Add a `mask` eval test where one source event has no overlap with the gate.
- Verify the non-overlapping event is dropped instead of leaking through.

**Step 2: Add a failing proof stub**

- Create `proofs/mask_spans.rs`.
- Define the span model and one deliberately failing lemma stub for overlap clipping or merged coverage.

**Step 3: Verify red**

Run:

- `cargo test -p orpheus-lang --test eval mask_drops`
- `C:\Users\markm\verus\verus.exe proofs/mask_spans.rs`

Expected:

- the Rust regression test should pass or fail honestly depending on current runtime behavior
- the Verus proof must fail until the lemmas are implemented

### Task 2: Prove The Mask Span Algebra

**Files:**
- Modify: `proofs/mask_spans.rs`

**Step 1: Prove merged coverage**

- Adjacent or overlapping sorted spans merge into one valid span.
- Coverage is preserved by the merge.

**Step 2: Prove clipped overlap boundaries**

- Clipped overlap start/end stay inside the source span.
- If overlap exists, the clipped span is valid.
- If no overlap exists, there is no clipped fragment.

**Step 3: Prove emitted-fragment safety**

- Any clipped fragment lies inside both source and gate spans.
- Any covered slot in the clipped span is covered by both source and gate.

### Task 3: Verify Green

**Files:**
- Modify: `proofs/mask_spans.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`

**Step 1: Run focused verification**

Run:

- `cargo test -p orpheus-lang --test eval mask_`
- `C:\Users\markm\verus\verus.exe proofs/mask_spans.rs`

Expected: PASS

### Task 4: Full Verification

**Files:**
- Modify: `proofs/mask_spans.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`

**Step 1: Run crate and proof verification**

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-lang --all-targets`
- `C:\Users\markm\verus\verus.exe proofs/mask_spans.rs`
- `C:\Users\markm\verus\verus.exe proofs/pattern_time.rs`
- `C:\Users\markm\verus\verus.exe proofs/cycle_query.rs`
- `C:\Users\markm\verus\verus.exe proofs/pitch_degrees.rs`

Expected: PASS
