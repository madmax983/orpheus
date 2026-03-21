# Mask Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `mask(gate, pattern)` to `orpheus-lang` as a fragment-level structural rhythm gate based on event overlap.

**Architecture:** Extend the builtin/type surface with `mask`, represent the gate as a runtime enum that can hold either sample or number patterns, and reuse the existing event-fragment machinery in `value.rs` to intersect source events with merged gate spans. The gate values are ignored; only occupancy matters.

**Tech Stack:** Rust 2024, `orpheus-lang`, `orpheus-pattern`, existing builtin evaluator and runtime fragment helpers

---

### Task 1: Add Red Tests

**Files:**
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/tests/infer.rs`

**Step 1: Add failing eval tests**

- `mask` keeps only source fragments overlapping the gate.
- pipe and direct-call forms match.
- number-pattern gates behave like sample-pattern gates.
- adjacent gate events merge into one open region.
- parameterized bindings work with `mask`.
- non-pattern gates fail clearly.

**Step 2: Add failing infer tests**

- partially applying `mask(gate)` infers a unary pattern function,
- `mask(..., pattern)` preserves sample-pattern types,
- number-pattern gates infer correctly,
- parameterized bindings with `mask` infer correctly.

**Step 3: Verify the red failure**

Run:

- `cargo test -p orpheus-lang --test infer mask_`
- `cargo test -p orpheus-lang --test eval mask_`

Expected: FAIL because `mask` is unresolved.

### Task 2: Add Builtin And Type Surface

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`
- Modify: `crates/orpheus-lang/src/value.rs`

**Step 1: Add builtin registration**

- Add `BuiltinKind::Mask`.
- Register `"mask"` in builtin lookup.
- Give it arity `2`.

**Step 2: Add the type scheme**

- Add `mask : Pattern<a> -> Pattern<b> -> Pattern<b>` using two quantified type variables.

**Step 3: Add builtin argument checking**

- First argument must be a pattern gate.
- Second argument must be a source pattern.
- Preserve clear diagnostics for bad gate shapes.

### Task 3: Implement Runtime Masking

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`

**Step 1: Add gate runtime storage**

- Add a `GatePatternRuntime` enum for sample/number gates.
- Add `PatternRuntime::Mask { gate, inner }`.
- Add `SamplePatternValue::mask(...)` and `NumberPatternValue::mask(...)`.

**Step 2: Reuse fragment composition**

- Query gate events over the requested span.
- Convert them into open spans and merge touching regions.
- Reuse the fragment helper to keep only overlapping source fragments.

**Step 3: Preserve non-value semantics**

- Ignore gate values entirely.
- Merge adjacent gate spans so a dense gate does not impose unwanted subdivision.

### Task 4: Verify Green

Run:

- `cargo test -p orpheus-lang --test infer mask_`
- `cargo test -p orpheus-lang --test eval mask_`

Expected: PASS

### Task 5: Full Crate Verification

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-lang --all-targets`

Expected: PASS
