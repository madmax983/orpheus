# Pitch Class Sets Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add first-class `pitch_class_set(...)` values and migrate `degrees(...)` from string-based collection lookup to value-based pitch collection semantics.

**Architecture:** Introduce a new runtime and HM type for pitch-class sets, preload canonical collections as built-in values, and make `degrees(...)` consume only first-class pitch-class-set values. Remove the old `degrees("name", ...)` string path instead of preserving dual semantics. Keep the existing signed octave-carry mapping and `transpose(...)` behavior intact.

**Tech Stack:** Rust 2024, `orpheus-lang`, existing parser/eval/infer/export tests, JSON fixture tests

---

### Task 1: Write Red Tests For The New Surface

**Files:**
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/tests/infer.rs`
- Modify: `crates/orpheus-lang/src/export.rs`

**Step 1: Add failing infer tests**

- `hirajoshi = pitch_class_set(0 2 3 7 8)` infers `PitchClassSet`.
- `melody = degrees(aeolian, 0 2 4)` infers `Pattern<Number>`.
- `melody = degrees(hirajoshi, 0 1 2 4)` infers `Pattern<Number>`.
- `degrees("aeolian", 0 2 4)` fails once the string path is removed.

**Step 2: Add failing eval tests**

- `pitch_class_set(0 2 3 7 8)` evaluates successfully.
- `degrees(aeolian, 0 2 4 7 8)` yields the same semitone output the old string path produced.
- `degrees(hirajoshi, 0 1 2 4)` maps through the user-defined set correctly.
- invalid sets reject clearly:
  - non-rooted
  - duplicate
  - unordered
  - out of range
  - fractional
- `degrees("aeolian", ...)` now errors with guidance toward value syntax.

**Step 3: Add a failing export regression**

- Export a parameterized melody built from a user-defined pitch-class set and compare against a fixture.

**Step 4: Verify red**

Run:

- `cargo test -p orpheus-lang --test infer pitch_class_set`
- `cargo test -p orpheus-lang --test eval pitch_class_set`
- `cargo test -p orpheus-lang pitch_class_set_export`

Expected:

- the new tests fail because `PitchClassSet` and `pitch_class_set(...)` do not exist yet
- any surviving string-based `degrees(...)` behavior becomes visible as an intentional failure to remove

### Task 2: Add Runtime And Type Representations

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/types/mod.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`

**Step 1: Add the runtime value**

- Introduce `PitchClassSetValue`.
- Add `Value::PitchClassSet(PitchClassSetValue)`.
- Add helpers such as `as_pitch_class_set()` and update `kind_name()`.

**Step 2: Replace `DegreeCollection`**

- Remove or demote the current `DegreeCollection` enum/string registry.
- Re-express canonical collections as `PitchClassSetValue` constants or constructors.

**Step 3: Add the HM type**

- Introduce `Type::PitchClassSet`.
- Update type display tests for the new type.
- Register canonical built-ins (`ionian`, `dorian`, `phrygian`, `mixolydian`, `aeolian`, `minor_pentatonic`) as monomorphic `PitchClassSet`.

### Task 3: Add Builtin Surface And Validation

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/eval.rs`

**Step 1: Register the constructor and built-in values**

- Add builtin function `pitch_class_set`.
- Inject canonical pitch-class-set values into `builtin_value(...)`.

**Step 2: Define validation**

- `pitch_class_set(...)` must accept whole-number numeric events only.
- the first pitch class must be `0`
- values must be strictly increasing
- values must stay within `[0, 11]`

**Step 3: Update `degrees(...)`**

- remove string extraction from `apply_degrees(...)`
- require a `PitchClassSetValue`
- keep integer-degree validation on the second argument
- keep the existing signed octave-carry mapping behavior

**Step 4: Add migration diagnostics**

- if the first argument to `degrees(...)` is a string, fail with a specific error like:
  - `` `degrees` now requires a pitch-class-set value such as `aeolian` or `pitch_class_set(...)` ``

### Task 4: Wire Canonical Collections Through The System

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`

**Step 1: Define canonical pitch-class-set values**

- `ionian`
- `dorian`
- `phrygian`
- `mixolydian`
- `aeolian`
- `minor_pentatonic`

**Step 2: Keep behavior stable**

- `degrees(aeolian, 0 2 4 7 8)` must produce the same semitone pattern the old string form produced
- `transpose(...)` should require no semantic changes

### Task 5: Add Export Fixture And Green Verification

**Files:**
- Create: `tests/fixtures/pitch_class_set_melody_export.json`
- Modify: `crates/orpheus-lang/src/export.rs`

**Step 1: Create the expected fixture**

- Use a small user-defined pitch-class set, a parameterized melodic helper, and `transpose(...)`.

Example shape:

```orpheus
walk set pat = degrees(set, pat) |> transpose(60)
hirajoshi = pitch_class_set(0 2 3 7 8)
melody = walk(hirajoshi)(0 1 2 4 5)
```

**Step 2: Verify focused green**

Run:

- `cargo test -p orpheus-lang --test infer pitch_class_set`
- `cargo test -p orpheus-lang --test eval pitch_class_set`
- `cargo test -p orpheus-lang pitch_class_set_export`

Expected: PASS

### Task 6: Full Verification

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/types/mod.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/tests/infer.rs`
- Modify: `crates/orpheus-lang/src/export.rs`

**Step 1: Format and lint**

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`

**Step 2: Run crate verification**

Run:

- `cargo test -p orpheus-lang --all-targets`

**Step 3: Re-run existing proofs**

Run:

- `C:\Users\markm\verus\verus.exe proofs\euclid_balance.rs`
- `C:\Users\markm\verus\verus.exe proofs\when_cycles.rs`
- `C:\Users\markm\verus\verus.exe proofs\within_windows.rs`
- `C:\Users\markm\verus\verus.exe proofs\mask_spans.rs`
- `C:\Users\markm\verus\verus.exe proofs\pattern_time.rs`
- `C:\Users\markm\verus\verus.exe proofs\cycle_query.rs`
- `C:\Users\markm\verus\verus.exe proofs\pitch_degrees.rs`

Expected: PASS

### Task 7: Follow-Up Proof Slice

**Files:**
- Create later: `proofs/pitch_class_sets.rs`

**Step 1: Defer formal validation**

- prove rooted-at-zero
- prove strict monotonicity
- prove octave-local bounds

This is intentionally out of scope for the implementation batch unless the runtime design becomes unstable without it.
