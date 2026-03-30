# Chord Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `chord(root, intervals)` to `orpheus-lang` as the first harmonic layer, returning stacked `Pattern<Number>` pitch events.

**Architecture:** Add a new builtin and a small runtime pattern node that combines a time-varying root number pattern with a unit-cycle interval set extracted from the second argument. Keep the output as a normal number pattern so inference, transforms, export, and JSON snapshot coverage all reuse existing machinery.

**Tech Stack:** Rust, `orpheus-lang`, existing `PatternRuntime<f64>` query model, JSON export tests.

---

### Task 1: Add red infer/eval/export tests for `chord(...)`

**Files:**
- Modify: `crates/orpheus-lang/tests/infer.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/src/export.rs`

**Step 1: Write the failing infer tests**

Add tests for:

- `pad = chord(c4, 0 4 7)` infers `Pattern<Number>`
- `line = chord(c4 e4, 0 7)` infers `Pattern<Number>`
- `harm = chord(degrees(aeolian, 0 2) |> transpose(60), 0 3 7)` infers `Pattern<Number>`

**Step 2: Write the failing eval tests**

Add tests for:

- `chord(c4, 0 4 7)` yields `60, 64, 67` over one span
- `chord(c4, 0 3 7 10)` yields `60, 63, 67, 70`
- `chord(c4 e4, 0 7)` yields dyads per root event
- `chord(...) |> transpose(12)` composes correctly

**Step 3: Write the failing export test**

Add a JSON fixture-backed export test for a small chord progression, for example:

```orpheus
pads = chord(c4 e4, 0 7)
```

**Step 4: Run the focused red tests**

Run:

- `cargo test -p orpheus-lang --test infer chord_`
- `cargo test -p orpheus-lang --test eval chord_`
- `cargo test -p orpheus-lang chord_export`

Expected: failures because `chord` does not exist yet.

### Task 2: Add the builtin surface and type scheme

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`

**Step 1: Add the builtin kind**

Extend `BuiltinKind` with `Chord`.

**Step 2: Register the builtin name**

Expose `chord` in `builtin_value(...)`.

**Step 3: Add the HM type**

Register:

```text
chord : Pattern<Number> -> Pattern<Number> -> Pattern<Number>
```

in `TypeEnv::with_builtins()`.

**Step 4: Run the infer red tests again**

Run:

- `cargo test -p orpheus-lang --test infer chord_`

Expected: still red until runtime/eval behavior exists, but type lookup should be on the right path.

### Task 3: Add runtime support for chord composition

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/builtins.rs`

**Step 1: Add a runtime pattern variant**

Add a new `PatternRuntime<f64>` variant like:

```rust
Chord {
    intervals: Vec<f64>,
    inner: Box<Self>,
}
```

where `inner` is the root pattern and `intervals` is the extracted unit-cycle set.

**Step 2: Add a constructor on `NumberPatternValue`**

Something like:

```rust
pub(crate) fn chord(self, intervals: Vec<f64>) -> Self
```

**Step 3: Implement query behavior**

When queried:

- query the root events normally
- for each root event, emit one event per interval value
- keep each emitted event on the same span as the root event
- `value = root + interval`
- sort output events before returning

**Step 4: Add interval-set extraction in `builtins.rs`**

For the second `chord` argument:

- extract a numeric pattern
- query it over `TimeSpan::unit()`
- collect the event values in order
- validate finite numeric values
- keep duplicates if present

**Step 5: Implement `apply_chord(args)`**

Steps:

- extract root number pattern
- extract interval-set values
- return `Value::NumberPattern(root_pattern.chord(intervals))`

**Step 6: Run the focused eval tests**

Run:

- `cargo test -p orpheus-lang --test eval chord_`

Expected: green.

### Task 4: Add export fixture coverage

**Files:**
- Modify: `crates/orpheus-lang/src/export.rs`
- Create: `tests/fixtures/chord_progression_export.json`

**Step 1: Add the exporter test**

Use a small root sequence:

```orpheus
pads = chord(c4 e4, 0 7)
```

**Step 2: Commit the expected JSON fixture**

Expected pitches:

- first half: `60`, `67`
- second half: `64`, `71`

**Step 3: Run the export test**

Run:

- `cargo test -p orpheus-lang chord_export`

Expected: green.

### Task 5: Full verification and cleanup

**Files:**
- Review touched files only

**Step 1: Scan the touched area for stubs**

Run:

- `rg -n "TODO|FIXME|Stub:" crates/orpheus-lang/src/value.rs crates/orpheus-lang/src/builtins.rs crates/orpheus-lang/src/types/env.rs crates/orpheus-lang/tests/infer.rs crates/orpheus-lang/tests/eval.rs crates/orpheus-lang/src/export.rs`

**Step 2: Run format, lint, and tests**

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-lang --all-targets`

**Step 3: Review the diff**

Confirm:

- no new runtime harmony type
- no chord-quality naming system
- interval sets stay unit-cycle data in v1
- exports still flow through normal number-pattern JSON

**Step 4: Commit**

```bash
git add crates/orpheus-lang/src/value.rs crates/orpheus-lang/src/builtins.rs crates/orpheus-lang/src/types/env.rs crates/orpheus-lang/tests/infer.rs crates/orpheus-lang/tests/eval.rs crates/orpheus-lang/src/export.rs tests/fixtures/chord_progression_export.json docs/plans/2026-03-21-chord-design.md docs/plans/2026-03-21-chord-implementation-plan.md
git commit -m "Add interval-stack chord patterns"
```

### Task 6: Optional proof backfill

**Files:**
- Create later: `proofs/chord_intervals.rs`

**Step 1: Prove the arithmetic spine**

Pin:

- `root + interval` mapping
- representative triad/seventh examples
- transpose difference preservation over chord notes

This is optional for the first implementation batch, but it is the obvious formal follow-up.
