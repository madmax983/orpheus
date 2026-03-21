# Within Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a cycle-local `within(start, end, transform)` combinator with exact rational windows and unary callable support for both builtin and user-defined transforms.

**Architecture:** Extend the builtin set and type environment with `within`, generalize transform storage from builtin-only to generic callable functions, and reuse localized cycle querying to implement window-local transform application in the runtime. The feature should stay cycle-local, constant-window-only, and exact in rational time.

**Tech Stack:** Rust 2024, existing `orpheus-lang` parser/eval/type runtime, `orpheus-pattern` rational spans, workspace test harness

---

### Task 1: Add The Failing Tests

**Files:**
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/tests/infer.rs`

**Step 1: Write the failing eval tests**

Add tests for:

- `within` reverses only the selected cycle window.
- pipe and direct-call forms match.
- user-defined unary transforms work inside `within`.
- invalid windows fail clearly.

**Step 2: Write the failing infer tests**

Add tests proving:

- `within(0, 0.5, rev, bd sn)` infers `Pattern<Sample>`.
- a parameterized unary transform inside `within` still infers correctly.

**Step 3: Run the targeted tests to verify they fail**

Run:

- `cargo test -p orpheus-lang --test eval within_`
- `cargo test -p orpheus-lang --test infer within_`

Expected: FAIL because `within` does not exist yet.

### Task 2: Add Builtin And Type Surface

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`

**Step 1: Add the builtin enum and type entry**

- Add `BuiltinKind::Within`.
- Give it arity `4`.
- Add a polymorphic `within` scheme to the type environment.

**Step 2: Generalize unary transform extraction**

- Change transform extraction from builtin-only to generic unary callable.
- Keep the same diagnostics shape used by `every` and `sometimes`.

**Step 3: Run the targeted infer tests**

Run:

- `cargo test -p orpheus-lang --test infer within_`

Expected: still FAIL on runtime behavior, but type surface compiles and inference works.

### Task 3: Generalize Transform Runtime Storage

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/eval.rs`

**Step 1: Replace builtin-only cycle transform storage**

- Change `PatternRuntime::Every` and `PatternRuntime::Sometimes` to store generic `FunctionValue`.
- Update `SamplePatternValue` and `NumberPatternValue` helpers accordingly.
- Add a small helper for applying a stored unary callable to a runtime value during querying.

**Step 2: Keep existing `every`/`sometimes` semantics intact**

- Reuse the old localization behavior; only widen accepted transform kind.

**Step 3: Run existing transform tests**

Run:

- `cargo test -p orpheus-lang every_`
- `cargo test -p orpheus-lang sometimes_`

Expected: PASS

### Task 4: Implement Window-Localized Querying

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/builtins.rs`

**Step 1: Add `PatternRuntime::Within`**

- Store exact rational `start`/`end`.
- Store generic unary `FunctionValue`.

**Step 2: Implement window localization**

- Build helpers to localize a full window to unit space.
- Query unaffected outside fragments directly.
- Apply the transform only to the localized window.
- Re-embed transformed events into the absolute window with rational scaling.

**Step 3: Validate constant windows**

- Accept only constant numeric windows.
- Reject values outside `[0, 1]`.
- Reject `start >= end`.

**Step 4: Run targeted eval tests**

Run:

- `cargo test -p orpheus-lang --test eval within_`

Expected: PASS

### Task 5: Verify The Whole Language Crate

**Files:**
- No code changes expected

**Step 1: Run formatting**

Run:

- `cargo fmt --all`

**Step 2: Run clippy**

Run:

- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`

**Step 3: Run crate tests**

Run:

- `cargo test -p orpheus-lang --all-targets`

Expected: PASS
