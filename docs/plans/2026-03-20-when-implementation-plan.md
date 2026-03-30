# When Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add `when(period, offset, transform)` to `orpheus-lang` as an offset-aware cycle-conditional transform combinator.

**Architecture:** Extend the builtin/type surface with `when`, add a runtime node for offset-aware cycle transforms, and reuse the existing localized cycle-query machinery already shared by `every` and `sometimes`. Keep V1 constant-only for `period` and `offset`, and keep the transform slot generic over unary callables so user-defined parameterized bindings work immediately.

**Tech Stack:** Rust 2024, `orpheus-lang`, `orpheus-pattern`, existing builtin evaluator and type environment

---

### Task 1: Add And Verify Red Tests

**Files:**
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/tests/infer.rs`

**Step 1: Add failing infer tests**

- `when` infers a unary transform when partially applied.
- `when(..., pattern)` preserves sample-pattern types.
- `when` accepts parameterized unary transforms.

**Step 2: Add failing eval tests**

- `when(3, 1, rev)` transforms only cycle offset `1`.
- direct-call and pipe forms match.
- parameterized unary transforms work inside `when`.
- invalid offsets reject clearly.

**Step 3: Run the targeted tests**

Run:

- `cargo test -p orpheus-lang --test infer when_`
- `cargo test -p orpheus-lang --test eval when_`

Expected: FAIL because `when` is unresolved.

### Task 2: Add Builtin And Type Surface

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`

**Step 1: Add the builtin**

- Add `BuiltinKind::When`.
- Register `"when"` in builtin lookup.
- Give it arity `4`.

**Step 2: Add the polymorphic type scheme**

- Mirror the `within`/`every` style by adding a curried type scheme for `when`.

**Step 3: Parse and validate constant numeric arguments**

- Reuse the existing constant-number helpers.
- Require `period > 0`.
- Require whole-number `offset`.
- Require `offset < period`.

### Task 3: Implement Runtime Querying

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`

**Step 1: Add runtime storage**

- Add `PatternRuntime::When { period, offset, transform, inner }`.
- Add `SamplePatternValue::when(...)` and `NumberPatternValue::when(...)`.

**Step 2: Reuse cycle-local querying**

- Route `PatternRuntime::When` through `query_transform_cycles(...)`.
- Use the predicate `cycle.rem_euclid(period) == offset`.
- Continue using absolute cycle numbers so nested transforms behave correctly.

### Task 4: Verify Green

**Files:**
- No new files expected

Run:

- `cargo test -p orpheus-lang --test infer when_`
- `cargo test -p orpheus-lang --test eval when_`

Expected: PASS

### Task 5: Full Crate Verification

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-lang --all-targets`

Expected: PASS
