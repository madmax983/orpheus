# Degrees Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add canonical pitch collections via `degrees(name, pattern)` and semitone shifting via `transpose(offset, pattern)` to `orpheus-lang`, with a Verus proof spine for signed degree mapping.

**Architecture:** Extend the builtin/type surface with two numeric-pattern builtins. Implement them as number-pattern runtime nodes so they compose with existing pattern transforms. Add `proofs/pitch_degrees.rs` to verify the signed Euclidean degree mapping and transposition invariants instead of leaving the pitch math as unproved folklore.

**Tech Stack:** Rust 2024, `orpheus-lang`, `orpheus-pattern`, Verus, existing JSON export fixture tests

---

### Task 1: Write Red Tests And Proof Skeleton

**Files:**
- Create: `proofs/pitch_degrees.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/tests/infer.rs`
- Modify: `crates/orpheus-lang/src/export.rs`

**Step 1: Add failing infer tests**

- `degrees("aeolian", 0 2 4)` infers `Pattern<Number>`.
- `transpose(45, degrees("aeolian", 0 2 4))` infers `Pattern<Number>`.

**Step 2: Add failing eval tests**

- `degrees("aeolian", 0 2 4 7 8)` yields `0 3 7 12 14`.
- `degrees("aeolian", -2 -1 0 1)` maps negative degrees correctly.
- `transpose(45, ...)` shifts every event by `45`.
- pattern-valued `transpose(12 -12, ...)` composes across fragments.
- unknown collections reject clearly.
- fractional degree events reject clearly.

**Step 3: Add a failing export fixture test**

- Export a parameterized melodic binding using `degrees(...) |> transpose(...)` and compare against a fixture.

**Step 4: Add the proof skeleton**

- Create `proofs/pitch_degrees.rs`.
- Define the collection interval model and the degree-to-semitone spec surface.
- Add lemma stubs for index bounds, signed octave carry, and transpose preservation.

**Step 5: Verify the red failure**

Run:

- `cargo test -p orpheus-lang --test infer degrees_`
- `cargo test -p orpheus-lang --test eval degrees_`
- `cargo test -p orpheus-lang degrees_export`
- `C:\Users\markm\verus\verus.exe proofs/pitch_degrees.rs`

Expected:

- the Rust tests fail because the builtins are unresolved
- the Verus file fails until the proof lemmas are implemented correctly

### Task 2: Add Builtin And Type Surface

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`
- Modify: `crates/orpheus-lang/src/value.rs`

**Step 1: Register the builtins**

- Add `BuiltinKind::Degrees`.
- Add `BuiltinKind::Transpose`.
- Register `"degrees"` and `"transpose"` in builtin lookup.
- Set arities to `2`.

**Step 2: Add type schemes**

- `degrees : String -> Pattern<Number> -> Pattern<Number>`
- `transpose : Pattern<Number> -> Pattern<Number> -> Pattern<Number>`

**Step 3: Add runtime nodes**

- Add a runtime node for degree mapping over number patterns.
- Add constant and pattern-valued runtime nodes for transposition over number patterns.

### Task 3: Implement Canonical Collection Mapping

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/value.rs`

**Step 1: Add the collection catalog**

- Hardcode the canonical interval tables for:
  - `ionian`
  - `dorian`
  - `phrygian`
  - `mixolydian`
  - `aeolian`
  - `minor_pentatonic`

**Step 2: Validate arguments**

- `degrees` requires a string collection name.
- `degrees` requires integer-valued numeric events.
- unknown names fail with a clear error.
- `transpose` requires finite numeric offsets.

**Step 3: Implement runtime evaluation**

- Map each number event through the chosen interval table using signed floor-division semantics.
- Add semitone offsets for `transpose`.
- Preserve spans and event ordering.

### Task 4: Implement The Verus Spine

**Files:**
- Modify: `proofs/pitch_degrees.rs`

**Step 1: Prove the interval table bounds**

- each canonical interval lies in `[0, 12)`
- interval tables are ordered

**Step 2: Prove signed degree mapping safety**

- Euclidean remainder stays inside table bounds
- negative degrees carry downward by octaves correctly
- positive overflow carries upward by octaves correctly

**Step 3: Prove transposition preserves interval structure**

- if two mapped semitone values differ by `d`, transposing both by `t` preserves the difference

### Task 5: Add Fixture And Green Verification

**Files:**
- Create: `tests/fixtures/degrees_melody_export.json`
- Modify: `crates/orpheus-lang/src/export.rs`

**Step 1: Create the expected JSON fixture**

- Use a parameterized melodic binding that exercises `degrees` plus `transpose`.

**Step 2: Verify focused green**

Run:

- `cargo test -p orpheus-lang --test infer degrees_`
- `cargo test -p orpheus-lang --test eval degrees_`
- `cargo test -p orpheus-lang degrees_export`
- `C:\Users\markm\verus\verus.exe proofs/pitch_degrees.rs`

Expected: PASS

### Task 6: Full Verification

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/tests/infer.rs`
- Modify: `crates/orpheus-lang/src/export.rs`
- Modify: `proofs/pitch_degrees.rs`

**Step 1: Run formatter and lints**

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`

**Step 2: Run crate verification**

Run:

- `cargo test -p orpheus-lang --all-targets`

**Step 3: Re-run proofs**

Run:

- `C:\Users\markm\verus\verus.exe proofs/pitch_degrees.rs`
- `C:\Users\markm\verus\verus.exe proofs/pattern_time.rs`
- `C:\Users\markm\verus\verus.exe proofs/cycle_query.rs`

Expected: PASS
