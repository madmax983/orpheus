# Roll Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `roll(steps, pattern)` as a polymorphic exact-span retrigger transform for sample and number patterns.

**Architecture:** Reuse the existing cluster-rewrite runtime pattern already used by `strum`, `arp`, `invert`, and `drop`. The type surface stays simple by reusing the same numeric-control-plus-pattern shape as `fast` and `slow`, while the proof spine focuses on equal partition coverage and repeated cluster replication.

**Tech Stack:** Rust 2024, `orpheus-lang`, `orpheus_pattern`, cargo test/clippy/fmt, Verus proofs in `proofs/`.

---

### Task 1: Add Failing Type Tests

**Files:**
- Modify: `crates/orpheus-lang/tests/infer.rs`

**Step 1: Write the failing tests**

Add tests for:

- `roll(4, sn)` infers `Pattern<Sample>`
- `roll(4, chord(c4, 0 4 7))` infers `Pattern<Number>`
- `bd |> roll(4)` or `chord(...) |> roll(4)` pipe form infers correctly
- `roll(4, c4)` or another bad non-pattern argument rejects clearly if needed

**Step 2: Run test to verify it fails**

Run: `cargo test -p orpheus-lang --test infer roll_`

Expected: FAIL because `roll` is unresolved in the builtin/type environment.

**Step 3: Do not write production code yet**

Stop after confirming the red state.

### Task 2: Add Failing Runtime And Export Tests

**Files:**
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/src/export.rs`
- Create: `tests/fixtures/roll_progression_export.json`

**Step 1: Write the failing runtime tests**

Add tests for:

- `roll(4, sn)` repeats a sample hit across four equal subspans
- `roll(4, chord(c4, 0 4 7))` repeats the full triad on each quarter
- direct call vs pipe form match
- `roll(1, pattern)` behaves as identity
- zero, negative, fractional, and non-constant `steps` reject clearly

**Step 2: Write the failing export snapshot test**

Add a JSON export test for one representative `roll(...)` pattern, likely a rolled chord or sample hit.

**Step 3: Run tests to verify they fail**

Run:

- `cargo test -p orpheus-lang --test eval roll_`
- `cargo test -p orpheus-lang roll_export`

Expected: FAIL because `roll` is unresolved.

### Task 3: Add The Type Environment And Builtin Surface

**Files:**
- Modify: `crates/orpheus-lang/src/types/env.rs`
- Modify: `crates/orpheus-lang/src/builtins.rs`

**Step 1: Add the builtin type**

Register `roll` in `TypeEnv::with_builtins()` using the same polymorphic shape as numeric-control pattern transforms:

```text
roll : Pattern<Number> -> Pattern<a> -> Pattern<a>
```

Reuse the existing helper if it already matches.

**Step 2: Add the builtin runtime entry**

Wire `roll` into:

- `builtin_value(...)`
- `BuiltinKind`
- builtin `name()`
- builtin `arity()`
- builtin `execute(...)`

**Step 3: Add minimal builtin execution**

Add `apply_roll(args)` in `builtins.rs`:

- extract constant positive whole-number `steps`
- dispatch on sample vs number pattern
- call new runtime methods on the pattern values
- reject non-pattern values with the usual diagnostics

**Step 4: Run type tests**

Run: `cargo test -p orpheus-lang --test infer roll_`

Expected: PASS once the type surface is wired.

### Task 4: Implement Runtime Cluster Retriggering

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`

**Step 1: Add runtime method shells**

Add:

- `SamplePatternValue::roll(self, steps: u32) -> Self`
- `NumberPatternValue::roll(self, steps: u32) -> Self`
- `PatternRuntime::{Roll { steps, inner }}`
- `PatternRuntimeValue::roll_events(...)`

**Step 2: Implement the cluster rewrite**

For both sample and number paths:

- query inner events
- group by exact `part`
- if cluster span width is zero, keep the cluster unchanged
- otherwise divide the span into `steps` equal adjacent subspans
- copy every event in the cluster onto every slot subspan
- clear `whole`
- sort output

Add a shared helper like `roll_event_cluster(...)` if possible.

**Step 3: Preserve existing runtime invariants**

Update:

- `try_query(...)`
- `absolute_cycle_for_runtime(...)`
- any exhaustive `match` blocks over `PatternRuntime` or `Value`

**Step 4: Run runtime tests**

Run: `cargo test -p orpheus-lang --test eval roll_`

Expected: PASS.

### Task 5: Fill The JSON Fixture

**Files:**
- Modify: `tests/fixtures/roll_progression_export.json`

**Step 1: Generate the actual output**

Use the export test failure diff or inspect generated JSON.

**Step 2: Replace the placeholder fixture**

Commit the exact wrapped/retriggered output expected from the representative test case.

**Step 3: Verify export**

Run: `cargo test -p orpheus-lang roll_export`

Expected: PASS.

### Task 6: Add Verus Proof Spine

**Files:**
- Create: `proofs/roll_partitions.rs`

**Step 1: Write the proof model**

Model:

- positive step count
- equal adjacent slot partition over a normalized span
- full coverage of the original span
- repeated cluster copy count law at the slot level

**Step 2: Prove the core lemmas**

Prove:

- slot adjacency
- slot coverage from origin to origin + steps
- positive slot width
- cluster replication multiplies event count by `steps`

Keep the proof arithmetic-only and self-contained. Do not try to verify the full runtime implementation.

**Step 3: Run Verus**

Run: `C:\Users\markm\verus\verus.exe proofs\roll_partitions.rs`

Expected: `0 errors`.

### Task 7: Final Verification

**Files:**
- Review all touched files

**Step 1: Format**

Run: `cargo fmt --all`

Expected: no diff after rerun.

**Step 2: Lint**

Run: `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`

Expected: PASS.

**Step 3: Run crate tests**

Run: `cargo test -p orpheus-lang --all-targets`

Expected: PASS.

**Step 4: Run proof verification**

Run:

- `C:\Users\markm\verus\verus.exe proofs\roll_partitions.rs`
- optionally re-run adjacent timing proofs if this slice touches shared helpers

Expected: PASS.

**Step 5: Scan for local sludge**

Run a focused search in the touched area for:

- `TODO`
- `FIXME`
- accidental placeholder fixtures

Expected: no new scaffolding left behind.
