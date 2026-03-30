# When Cycle Proofs Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a Verus proof spine for `when(period, offset, transform)` and pin the matching/non-matching cycle partition with one focused runtime regression.

**Architecture:** Model `when` as a residue-class selector over integer cycle indices, then prove that the matching and non-matching cycle subsets form a total, disjoint partition of any queried cycle range. Keep the proof model smaller than the runtime and use the Rust regression only to pin observable multi-cycle behavior.

**Tech Stack:** Rust, Verus, `cargo test`, `cargo clippy`

---

### Task 1: Write the runtime regression first

**Files:**
- Modify: `crates/orpheus-lang/tests/eval.rs`

**Step 1: Write the failing/guardrail test**

Add a regression that evaluates:

```orpheus
drums = bd sn |> when(3, 1, rev)
```

and exports five cycles. Assert that:

- cycles `1` and `4` are reversed
- cycles `0`, `2`, and `3` remain in source order

**Step 2: Run the targeted test**

Run:

```bash
cargo test -p orpheus-lang when_applies_its_transform_on_cycles_one_and_four_in_a_five_cycle_query
```

Expected: pass if runtime already matches the theorem, otherwise fail with a concrete mismatch.

### Task 2: Add the red proof stub

**Files:**
- Create: `proofs/when_cycles.rs`

**Step 1: Write the initial spec skeleton**

Add:

- `valid_when`
- `cycle_in_query`
- `matches_when`

and one deliberately incomplete lemma for the partition law.

**Step 2: Run Verus to verify RED**

Run:

```bash
C:\Users\markm\verus\verus.exe proofs\when_cycles.rs
```

Expected: fail because the partition proof is still incomplete.

### Task 3: Fill in the proof spine

**Files:**
- Modify: `proofs/when_cycles.rs`

**Step 1: Add validity and periodicity lemmas**

Prove:

- valid parameters imply positive period
- matching repeats every `period`
- non-zero deltas smaller than `period` cannot preserve a match

**Step 2: Add query partition lemmas**

Prove:

- matching cycles in a query stay inside the query
- non-matching cycles in a query stay inside the query
- each queried cycle is exactly one of matching or non-matching
- matching and non-matching cannot both hold

**Step 3: Re-run Verus**

Run:

```bash
C:\Users\markm\verus\verus.exe proofs\when_cycles.rs
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
C:\Users\markm\verus\verus.exe proofs\when_cycles.rs
C:\Users\markm\verus\verus.exe proofs\within_windows.rs
C:\Users\markm\verus\verus.exe proofs\mask_spans.rs
C:\Users\markm\verus\verus.exe proofs\pattern_time.rs
C:\Users\markm\verus\verus.exe proofs\cycle_query.rs
C:\Users\markm\verus\verus.exe proofs\pitch_degrees.rs
```

Expected: all verify cleanly.
