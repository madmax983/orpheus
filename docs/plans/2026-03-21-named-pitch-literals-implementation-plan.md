# Named Pitch Literals Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add lowercase ASCII named pitch literals like `c4`, `fs4`, and `bf3` as absolute semitone-valued numeric patterns in `orpheus-lang`.

**Architecture:** Reuse the existing identifier grammar and AST. Recognize valid pitch-literal-shaped identifiers in the inference and evaluation identifier-resolution paths, lowering them into `Pattern<Number>` values without introducing a new runtime type. Keep export behavior unchanged by relying on the existing number-pattern pipeline.

**Tech Stack:** Rust, `orpheus-lang`, existing parser/eval/type inference layers, JSON export tests.

---

### Task 1: Add red tests for named pitch literals

**Files:**
- Modify: `crates/orpheus-lang/tests/parser.rs`
- Modify: `crates/orpheus-lang/tests/infer.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/src/export.rs`

**Step 1: Write the failing parser smoke test**

Add a parser test proving the token survives parsing in a sequence, for example:

```rust
#[test]
fn parses_named_pitch_literals_as_ident_tokens() {
    let expr = binding_expr("melody = c4 ef4 g4 bf4");
    assert!(matches!(expr, Expr::Seq(_)));
}
```

**Step 2: Write failing infer tests**

Add tests for:

- `melody = c4 ef4 g4 bf4` infers `Pattern<Number>`
- `riff = fs4 a4 cs5 |> fast(2)` infers `Pattern<Number>`
- `line = degrees(aeolian, 0 2 4) |> transpose(c4)` is not required
- malformed `cf` fails with a pitch-literal-aware diagnostic if recognized

**Step 3: Write failing eval tests**

Add tests for:

- `c4 => 60`
- `a4 => 69`
- `bf3 => 58`
- `fs4 => 66`
- `transpose(12, c4 e4 g4)` yields `72 76 79`

**Step 4: Write failing export fixture test**

Add a JSON snapshot-style export test for a named-pitch melody.

Example source:

```orpheus
melody = c4 ef4 g4 bf4
```

**Step 5: Run the focused red tests**

Run:

- `cargo test -p orpheus-lang --test infer named_pitch`
- `cargo test -p orpheus-lang --test eval named_pitch`
- `cargo test -p orpheus-lang named_pitch_export`

Expected: failures because no pitch-literal resolver exists yet.

### Task 2: Add a shared named-pitch parser helper

**Files:**
- Create: `crates/orpheus-lang/src/pitch.rs`
- Modify: `crates/orpheus-lang/src/lib.rs`

**Step 1: Add the helper API**

Create a small helper module with functions such as:

- `parse_named_pitch_literal(name: &str) -> Result<Option<i32>, PitchLiteralError>`
- optionally a helper for semitone mapping

**Step 2: Encode the v1 syntax**

Recognize:

- `[a-g]`
- optional `s` or `f`
- required decimal octave suffix

**Step 3: Encode the value mapping**

Map using:

```text
12 * (octave + 1) + pitch_class + accidental
```

**Step 4: Add unit tests in the new module**

Test:

- `c4 => 60`
- `a4 => 69`
- `bf3 => 58`
- `fs4 => 66`
- malformed `cf`

**Step 5: Run the helper tests**

Run:

- `cargo test -p orpheus-lang pitch_literal`

Expected: green for the helper in isolation.

### Task 3: Wire named pitches into type inference and evaluation

**Files:**
- Modify: `crates/orpheus-lang/src/types/infer.rs`
- Modify: `crates/orpheus-lang/src/eval.rs`

**Step 1: Update inference identifier resolution**

In `infer_ident(...)`:

- first honor user/builtin bindings as normal
- if unresolved, try `parse_named_pitch_literal(name)`
- if it returns a valid pitch, infer `Type::pattern(Type::Number)`
- if it returns a pitch-literal-specific error, surface that diagnostic

**Step 2: Update evaluation identifier resolution**

In `eval_ident(...)`:

- first honor user bindings and builtins as normal
- if unresolved, try `parse_named_pitch_literal(name)`
- if valid, return `Value::NumberPattern(NumberPatternValue::constant(...))`
- if malformed in a pitch-specific way, surface that diagnostic

**Step 3: Keep runtime minimal**

Do not add a new `Value` variant or AST node. Lower directly to the existing number-pattern path.

**Step 4: Run focused tests**

Run:

- `cargo test -p orpheus-lang --test infer named_pitch`
- `cargo test -p orpheus-lang --test eval named_pitch`

Expected: green.

### Task 4: Add export coverage and fixture

**Files:**
- Modify: `crates/orpheus-lang/src/export.rs`
- Create: `tests/fixtures/named_pitch_melody_export.json`

**Step 1: Add the direct exporter test**

Export:

```orpheus
melody = c4 ef4 g4 bf4
```

through `export_number_pattern_to_json(...)`.

**Step 2: Add the committed fixture**

Expected values:

- `60.0`
- `63.0`
- `67.0`
- `70.0`

over one cycle with equal subdivisions.

**Step 3: Run the focused export test**

Run:

- `cargo test -p orpheus-lang named_pitch_export`

Expected: green.

### Task 5: Full verification and cleanup

**Files:**
- Review touched files only

**Step 1: Scan the touched area for stubs**

Run:

- `rg -n "TODO|FIXME|Stub:" crates/orpheus-lang/src/pitch.rs crates/orpheus-lang/src/types/infer.rs crates/orpheus-lang/src/eval.rs crates/orpheus-lang/tests/parser.rs crates/orpheus-lang/tests/infer.rs crates/orpheus-lang/tests/eval.rs crates/orpheus-lang/src/export.rs`

Expected: no matches in the touched area.

**Step 2: Run format, lint, and tests**

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-lang --all-targets`

**Step 3: Review the diff**

Confirm the slice stays small:

- no grammar churn
- no AST churn
- no new runtime `Pitch` type
- named pitches export through existing number-pattern JSON

**Step 4: Commit**

```bash
git add crates/orpheus-lang/src/pitch.rs crates/orpheus-lang/src/lib.rs crates/orpheus-lang/src/types/infer.rs crates/orpheus-lang/src/eval.rs crates/orpheus-lang/tests/parser.rs crates/orpheus-lang/tests/infer.rs crates/orpheus-lang/tests/eval.rs crates/orpheus-lang/src/export.rs tests/fixtures/named_pitch_melody_export.json docs/plans/2026-03-21-named-pitch-literals-design.md docs/plans/2026-03-21-named-pitch-literals-implementation-plan.md
git commit -m "Add named pitch literals"
```

### Task 6: Optional proof backfill

**Files:**
- Create later: `proofs/named_pitch_literals.rs`

**Step 1: Prove the semitone mapping spine**

Pin:

- pitch-class mapping
- accidental adjustment
- absolute-value examples like `c4 = 60` and `a4 = 69`

This is optional for the first implementation batch, but it is the obvious formal follow-up if we keep the feature.
