# Euclid Balance Proofs Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Verus proof spine for Euclidean gate balance and pin the current `euclid(5, 8)` runtime sequence with an evaluator regression.

**Architecture:** Model Euclidean rhythm balance with integer slot gaps rather than reproducing the recursive runtime builder in Verus. Prove the count, span, and gap-balance laws in `proofs/euclid_balance.rs`, and keep the runtime behavior anchored with one exact `euclid(5, 8)` test in the evaluator suite.

**Tech Stack:** Rust, Verus, `cargo test`, `cargo clippy`

---

### Task 1: Write the runtime regression first

**Files:**
- Modify: `crates/orpheus-lang/tests/eval.rs`

**Step 1: Write the new Euclid regression**

Add a test that evaluates:

```orpheus
clave = euclid(5, 8)
```

and asserts that the open slot starts are:

- `0/8`
- `2/8`
- `3/8`
- `5/8`
- `6/8`

**Step 2: Run the targeted test**

Run:

```bash
cargo test -p orpheus-lang euclid_generates_five_open_steps_in_eight -- --exact
```

Expected: either PASS immediately or FAIL with a concrete mismatch in the current builder sequence.

### Task 2: Add the red proof stub

**Files:**
- Create: `proofs/euclid_balance.rs`

**Step 1: Write the spec skeleton**

Add:

- `valid_euclid`
- `small_gap`
- `large_gap`
- `large_gap_count`
- `small_gap_count`
- a simple gap/onset model

and leave one proof unfinished on purpose.

**Step 2: Run Verus to verify RED**

Run:

```bash
C:\Users\markm\verus\verus.exe proofs\euclid_balance.rs
```

Expected: FAIL because the balance/count proof is still incomplete.

### Task 3: Fill in the proof spine

**Files:**
- Modify: `proofs/euclid_balance.rs`

**Step 1: Prove the count and span laws**

Prove:

- valid parameters bound pulses and steps
- `small_gap_count + large_gap_count == pulses`
- `small_gap_count * small_gap + large_gap_count * large_gap == steps`

**Step 2: Prove the gap-shape laws**

Prove:

- for `pulses > 0`, `small_gap` and `large_gap` are positive
- each modeled gap is either `small_gap` or `large_gap`
- `large_gap - small_gap <= 1`

**Step 3: Prove the onset-range laws**

Prove:

- onset slots are in range for indices `< pulses`
- onset slots are strictly increasing

**Step 4: Re-run Verus**

Run:

```bash
C:\Users\markm\verus\verus.exe proofs\euclid_balance.rs
```

Expected: `0 errors`

### Task 4: Run the full verification gate

**Files:**
- Modify only the files above if needed

**Step 1: Format**

Run:

```bash
cargo fmt --all
```

**Step 2: Lint**

Run:

```bash
cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings
```

**Step 3: Test**

Run:

```bash
cargo test -p orpheus-lang --all-targets
```

**Step 4: Re-run the proof suite**

Run:

```bash
C:\Users\markm\verus\verus.exe proofs\euclid_balance.rs
C:\Users\markm\verus\verus.exe proofs\when_cycles.rs
C:\Users\markm\verus\verus.exe proofs\within_windows.rs
C:\Users\markm\verus\verus.exe proofs\mask_spans.rs
C:\Users\markm\verus\verus.exe proofs\pattern_time.rs
C:\Users\markm\verus\verus.exe proofs\cycle_query.rs
C:\Users\markm\verus\verus.exe proofs\pitch_degrees.rs
```

Expected: all verify cleanly.
