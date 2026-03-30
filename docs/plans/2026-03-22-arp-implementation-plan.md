# Arp Transform Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `arp(steps, direction, pattern)` as a repeating exact-span harmonic sequencer for `Pattern<Number>`.

**Architecture:** Reuse the exact-span cluster law already used by `strum`, `invert`, and `drop`. Add a first-class `ArpDirection` value/type plus a number-pattern runtime rewrite that partitions each cluster into `steps` equal adjacent subspans and emits wrapped pitch traversal in the chosen direction.

**Tech Stack:** Rust, `orpheus-lang`, HM type environment, evaluator/runtime pattern machinery, JSON export tests, Verus spine proofs.

---

### Task 1: Write the failing arp tests

**Files:**
- Modify: `crates/orpheus-lang/tests/infer.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/src/export.rs`
- Create: `tests/fixtures/arp_progression_export.json`

**Step 1: Write the failing test**

Add coverage for:

- infer `arp(5, up, chord(c4, 0 4 7))`
- infer pipe form
- eval wrapped `up` traversal
- eval wrapped `down` traversal
- eval per-cluster behavior on sequential harmonic material
- eval single-note cluster repetition
- eval direct-call and pipe-form equivalence
- eval composition with `drop(...)` and/or `invert(...)`
- reject zero, negative, fractional, and non-constant step counts
- reject non-direction second arguments
- reject non-number-pattern final arguments
- export fixture for one arpeggiated progression

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p orpheus-lang --test infer arp_
cargo test -p orpheus-lang --test eval arp_
cargo test -p orpheus-lang arp_export
```

Expected: FAIL because `arp`, `up`, and `down` do not exist yet.

### Task 2: Add first-class direction values and builtin/type surface

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/types/mod.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`
- Modify as needed: `crates/orpheus-lang/src/types/infer.rs`
- Modify as needed: `crates/orpheus-lang/src/eval.rs`

**Step 1: Write the failing test**

Use the Task 1 infer failures as the red state.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p orpheus-lang --test infer arp_
```

Expected: FAIL because the new value/type surface is missing.

**Step 3: Write minimal implementation**

Add:

- `Type::ArpDirection`
- `Value::ArpDirection`
- builtin values `up` and `down`
- `BuiltinKind::Arp`
- HM env entries for `up`, `down`, and `arp`
- any required inference/evaluator exhaustiveness updates for the new value kind

### Task 3: Implement runtime arp traversal

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/builtins.rs`

**Step 1: Write the failing test**

Use the Task 1 eval failures as the red state.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p orpheus-lang --test eval arp_
```

Expected: FAIL because runtime cluster traversal is missing.

**Step 3: Write minimal implementation**

Implement:

- step extraction as a constant positive whole number
- direction extraction from first-class direction values
- `NumberPatternValue::arp(steps, direction)`
- exact-span cluster rewrite with equal adjacent subspans
- wrapped pitch selection for `up` and `down`
- zero-width cluster guard: leave unchanged

### Task 4: Add export snapshot coverage

**Files:**
- Modify: `crates/orpheus-lang/src/export.rs`
- Create: `tests/fixtures/arp_progression_export.json`

**Step 1: Write the failing test**

Add a direct JSON export regression for an arpeggiated progression.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p orpheus-lang arp_export
```

Expected: FAIL until the fixture matches the real output.

### Task 5: Backfill the proof spine

**Files:**
- Create: `proofs/arp_sequences.rs`

**Step 1: Write the failing test**

Start from a small arithmetic model:

- equal partitioning into `steps`
- wrapped index selection for `up`
- wrapped index selection for `down`

**Step 2: Run test to verify it fails**

Run:

```bash
C:\Users\markm\verus\verus.exe proofs/arp_sequences.rs
```

Expected: FAIL until the lemmas are written.

**Step 3: Write minimal implementation**

Prove:

- partition count equals `steps`
- partition subspans are adjacent and non-overlapping
- `up` traversal wraps modulo cluster size
- `down` traversal wraps modulo cluster size from the top
- example arpeggios match the runtime design

### Task 6: Verify and harden

**Files:**
- Modify as needed: touched `arp` files only

**Step 1: Write the failing test**

If verification exposes an edge case, add the failing regression first.

**Step 2: Run test to verify it fails**

Run focused commands for the discovered regression.

**Step 3: Write minimal implementation**

Fix only the actual edge case. Keep the cluster law small and deterministic.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo fmt --all
cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings
cargo test -p orpheus-lang --all-targets
C:\Users\markm\verus\verus.exe proofs/arp_sequences.rs
```

Also re-run adjacent proof files if the touched runtime area overlaps them.
