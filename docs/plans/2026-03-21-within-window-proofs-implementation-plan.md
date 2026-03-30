# Within Window Proofs Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Verus proofs for the window localization and restoration geometry behind `within`, plus one runtime boundary regression for a full-cycle window.

**Architecture:** Create a small proof file, `proofs/within_windows.rs`, that models valid windows, localized spans, restored spans, and their ordering/round-trip laws using integer half-open intervals. Keep the proof mathematical and local to the helper contracts already used by `query_within(...)`, and add one exact-boundary eval regression for `within(0, 1, ...)`.

**Tech Stack:** Rust 2024, Verus, `orpheus-lang` eval tests

---

### Task 1: Add The Red Slice

**Files:**
- Create: `proofs/within_windows.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`

**Step 1: Add a focused boundary regression**

- Add an eval test proving `within(0, 1, rev)` matches `rev(...)`.

**Step 2: Add a failing proof stub**

- Create `proofs/within_windows.rs`.
- Define the window/span model and one deliberately failing lemma stub for round-trip or containment.

**Step 3: Verify red**

Run:

- `cargo test -p orpheus-lang --test eval within_full_cycle`
- `C:\Users\markm\verus\verus.exe proofs/within_windows.rs`

Expected:

- the Rust regression may already pass if runtime behavior is already correct
- the Verus proof must fail until the lemmas are implemented

### Task 2: Prove The Within Window Geometry

**Files:**
- Modify: `proofs/within_windows.rs`

**Step 1: Prove validity and width**

- valid windows have positive width
- localizing an in-window span yields a valid local span
- restoring a valid local span yields a valid in-window span

**Step 2: Prove containment**

- localized endpoints stay inside `[0, width]`
- restored endpoints stay inside the original window

**Step 3: Prove round-trip and ordering**

- restoring a localized point returns the original point
- restoring a localized span returns the original span
- localization preserves ordering
- restoration preserves ordering

### Task 3: Verify Green

**Files:**
- Modify: `proofs/within_windows.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`

**Step 1: Run focused verification**

Run:

- `cargo test -p orpheus-lang --test eval within_`
- `C:\Users\markm\verus\verus.exe proofs/within_windows.rs`

Expected: PASS

### Task 4: Full Verification

**Files:**
- Modify: `proofs/within_windows.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`

**Step 1: Run crate and proof verification**

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-lang --all-targets`
- `C:\Users\markm\verus\verus.exe proofs/within_windows.rs`
- `C:\Users\markm\verus\verus.exe proofs/mask_spans.rs`
- `C:\Users\markm\verus\verus.exe proofs/pattern_time.rs`
- `C:\Users\markm\verus\verus.exe proofs/cycle_query.rs`
- `C:\Users\markm\verus\verus.exe proofs/pitch_degrees.rs`

Expected: PASS
