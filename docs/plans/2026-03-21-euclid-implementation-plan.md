# Euclid Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `euclid(pulses, steps)` to `orpheus-lang` as a reusable Euclidean rhythm gate generator returning `Pattern<Number>`.

**Architecture:** Extend the builtin/type surface with `euclid`, validate constant whole-number arguments at runtime, and generate a plain `NumberPatternValue` from `PatternNode` data. Use `mask` in tests and one JSON fixture to pin compositional behavior without adding any new runtime transform node.

**Tech Stack:** Rust 2024, `orpheus-lang`, `orpheus-pattern`, existing builtin evaluator and JSON export path

---

### Task 1: Add Red Tests

**Files:**
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/tests/infer.rs`
- Modify: `crates/orpheus-lang/src/export.rs`

**Step 1: Add failing infer tests**

- `euclid(3, 8)` infers `Pattern<Number>`.
- `mask(euclid(3, 8), pattern)` preserves the source pattern type.

**Step 2: Add failing eval tests**

- `euclid(3, 8)` yields exactly 3 open steps over the unit cycle.
- `euclid(0, 8)` yields no events.
- `euclid(8, 8)` yields 8 open steps.
- `mask(euclid(3, 8), ...)` keeps the expected slices.
- invalid pulses/steps reject clearly.

**Step 3: Add a failing export fixture test**

- Export a masked sample pattern driven by `euclid(3, 8)` and compare against a fixture.

**Step 4: Verify the red failure**

Run:

- `cargo test -p orpheus-lang --test infer euclid_`
- `cargo test -p orpheus-lang --test eval euclid_`
- `cargo test -p orpheus-lang euclid_export`

Expected: FAIL because `euclid` is unresolved.

### Task 2: Add Builtin And Type Surface

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`

**Step 1: Add builtin registration**

- Add `BuiltinKind::Euclid`.
- Register `"euclid"` in builtin lookup.
- Give it arity `2`.

**Step 2: Add the type scheme**

- Add `euclid : Pattern<Number> -> Pattern<Number> -> Pattern<Number>`.

### Task 3: Implement Euclidean Gate Generation

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`

**Step 1: Validate arguments**

- Require constant whole-number `pulses` and `steps`.
- Require `steps > 0`.
- Require `pulses <= steps`.

**Step 2: Generate the step vector**

- Use a deterministic Euclidean distribution algorithm.
- Convert open steps to `PatternNode::atom(1.0)`.
- Convert closed steps to `PatternNode::rest()`.

**Step 3: Return a plain `NumberPatternValue`**

- Use `NumberPatternValue::from_nodes(...)`.

### Task 4: Verify Green

Run:

- `cargo test -p orpheus-lang --test infer euclid_`
- `cargo test -p orpheus-lang --test eval euclid_`
- `cargo test -p orpheus-lang euclid_export`

Expected: PASS

### Task 5: Full Crate Verification

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-lang --all-targets`

Expected: PASS
