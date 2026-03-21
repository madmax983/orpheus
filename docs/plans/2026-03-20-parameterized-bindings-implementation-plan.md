# Parameterized Bindings Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add top-level parameterized bindings with curried parenthesized calls, user-function partial application, and generalized type inference in `orpheus-lang`.

**Architecture:** Extend the parser and AST so top-level bindings can declare parameters and expression parsing can chain postfix calls on any callable expression. Replace the narrow builtin-only runtime function value with a shared callable representation that can model both builtins and user-defined functions. Upgrade type inference from monomorphic top-level inserts to generalized schemes so user-defined curried bindings instantiate cleanly at each use site.

**Tech Stack:** Rust 2024, pest, existing `orpheus-lang` parser/eval/type infrastructure, workspace test harness

---

### Task 1: Parse Parameterized Bindings And Chained Calls

**Files:**
- Modify: `crates/orpheus-lang/src/ast.rs`
- Modify: `crates/orpheus-lang/src/grammar/orpheus.pest`
- Modify: `crates/orpheus-lang/src/parser.rs`
- Test: `crates/orpheus-lang/tests/parser.rs`

**Step 1: Write the failing tests**

Add parser tests for:

```rust
#[test]
fn parses_parameterized_binding_headers() {
    let module = parse_module("swing amt pat = pat |> shift(amt)").unwrap();
    match &module.statements[0] {
        Stmt::Binding { name, params, expr } => {
            assert_eq!(name, "swing");
            assert_eq!(params, &["amt".to_owned(), "pat".to_owned()]);
            assert!(matches!(expr, Expr::Pipe { .. }));
        }
    }
}

#[test]
fn parses_chained_curried_calls() {
    let module = parse_module("groove = swing(0.125)(bd sn)").unwrap();
    match &module.statements[0] {
        Stmt::Binding { expr, .. } => assert!(matches!(expr, Expr::Call { .. })),
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orpheus-lang --test parser parses_parameterized_binding_headers parses_chained_curried_calls`

Expected: FAIL with AST or grammar errors because bindings do not accept params and chained postfix calls are not parsed.

**Step 3: Write minimal implementation**

Update the AST and parser so:

```rust
pub enum Stmt {
    Binding {
        name: String,
        params: Vec<String>,
        expr: Expr,
    },
}
```

and expression parsing treats repeated argument lists as nested calls:

```text
swing(0.125)(bd sn)
=> Call {
     callee: Box::new(Call { callee: Ident("swing"), args: [Number(0.125)] }),
     args: [Seq([Ident("bd"), Ident("sn")])]
   }
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p orpheus-lang --test parser`

Expected: PASS

**Step 5: Commit**

```bash
git add crates/orpheus-lang/src/ast.rs crates/orpheus-lang/src/grammar/orpheus.pest crates/orpheus-lang/src/parser.rs crates/orpheus-lang/tests/parser.rs
git commit -m "feat(lang): parse parameterized bindings and chained calls"
```

### Task 2: Evaluate User-Defined Curried Functions

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/eval.rs`
- Test: `crates/orpheus-lang/tests/eval.rs`

**Step 1: Write the failing tests**

Add eval tests for:

```rust
#[test]
fn parameterized_binding_can_be_applied_curried() {
    let module = eval_module(
        "swing amt pat = pat |> shift(amt)\n\
         groove = swing(0.25)(bd sn)",
        ReplMode::Loose,
    )
    .unwrap();

    let events = module.get("groove").unwrap().as_sample_pattern().unwrap().query_unit().unwrap();
    assert_eq!(events[0].value.sample(), "sn");
}

#[test]
fn parameterized_binding_can_be_used_from_pipe() {
    let direct = eval_module(
        "swing amt pat = pat |> shift(amt)\n\
         groove = swing(0.25)(bd sn)",
        ReplMode::Loose,
    )
    .unwrap();
    let piped = eval_module(
        "swing amt pat = pat |> shift(amt)\n\
         groove = bd sn |> swing(0.25)",
        ReplMode::Loose,
    )
    .unwrap();

    assert_eq!(
        direct.get("groove").unwrap().as_sample_pattern().unwrap().query_unit().unwrap(),
        piped.get("groove").unwrap().as_sample_pattern().unwrap().query_unit().unwrap(),
    );
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orpheus-lang --test eval parameterized_binding_can_be_applied_curried parameterized_binding_can_be_used_from_pipe`

Expected: FAIL because top-level parameterized bindings do not produce callable runtime values.

**Step 3: Write minimal implementation**

Introduce a shared callable model:

```rust
pub enum Callable {
    Builtin(BuiltinFn),
    User(UserFn),
}

pub enum Value {
    SamplePattern(SamplePatternValue),
    NumberPattern(NumberPatternValue),
    Function(Callable),
    String(String),
}
```

`UserFn` should store:

- remaining parameter names
- function body AST
- captured binding environment

Evaluation rules:

- inserting a parameterized binding stores a callable instead of evaluating the body immediately
- application binds one argument at a time
- partial application returns another callable
- final saturation evaluates the stored body in the extended environment
- self-reference is rejected while building the function binding

**Step 4: Run tests to verify they pass**

Run: `cargo test -p orpheus-lang --test eval`

Expected: PASS

**Step 5: Commit**

```bash
git add crates/orpheus-lang/src/value.rs crates/orpheus-lang/src/builtins.rs crates/orpheus-lang/src/eval.rs crates/orpheus-lang/tests/eval.rs
git commit -m "feat(lang): evaluate user-defined curried bindings"
```

### Task 3: Generalize Parameterized Bindings In The Type System

**Files:**
- Modify: `crates/orpheus-lang/src/types/env.rs`
- Modify: `crates/orpheus-lang/src/types/infer.rs`
- Test: `crates/orpheus-lang/tests/infer.rs`

**Step 1: Write the failing tests**

Add inference tests for:

```rust
#[test]
fn parameterized_binding_infers_a_curried_function_type() {
    let typed = infer_module("swing amt pat = pat |> shift(amt)", ReplMode::Strict).unwrap();
    assert_eq!(
        typed.type_of("swing").to_string(),
        "Pattern<Number> -> Pattern<t0> -> Pattern<t0>"
    );
}

#[test]
fn parameterized_bindings_are_generalized_at_each_use_site() {
    let typed = infer_module(
        "id pat = pat\n\
         drums = id(bd sn)\n\
         cutoff = id(400 800)",
        ReplMode::Strict,
    )
    .unwrap();

    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
    assert_eq!(typed.type_of("cutoff").to_string(), "Pattern<Number>");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orpheus-lang --test infer parameterized_binding_infers_a_curried_function_type parameterized_bindings_are_generalized_at_each_use_site`

Expected: FAIL because parameterized user bindings are not generalized or even represented as functions.

**Step 3: Write minimal implementation**

When inferring a parameterized binding:

```rust
let param_types = params.iter().map(|_| self.fresh_var_type()).collect::<Vec<_>>();
// Insert params into a local environment, infer body, then wrap in curried function type.
let binding_ty = Type::curried(param_types, inferred_body_ty);
let scheme = self.generalize(binding_ty);
```

Generalization should quantify type variables not free in the surrounding environment, then store the resulting `TypeScheme` in `env`.

**Step 4: Run tests to verify they pass**

Run: `cargo test -p orpheus-lang --test infer`

Expected: PASS

**Step 5: Commit**

```bash
git add crates/orpheus-lang/src/types/env.rs crates/orpheus-lang/src/types/infer.rs crates/orpheus-lang/tests/infer.rs
git commit -m "feat(lang): generalize parameterized binding types"
```

### Task 4: Lock In Diagnostics And Regressions

**Files:**
- Modify: `crates/orpheus-lang/tests/parser.rs`
- Modify: `crates/orpheus-lang/tests/eval.rs`
- Modify: `crates/orpheus-lang/tests/infer.rs`

**Step 1: Write the failing tests**

Add regression and error tests for:

```rust
#[test]
fn duplicate_parameter_names_are_rejected() {
    let error = parse_module("swing amt amt = amt").unwrap_err();
    assert!(error.to_string().contains("duplicate"));
}

#[test]
fn self_recursive_parameterized_binding_is_rejected() {
    let error = eval_module("loop pat = loop(pat)", ReplMode::Strict).unwrap_err();
    assert!(error.to_string().contains("self-reference"));
}

#[test]
fn sequencing_syntax_still_parses_as_sequence() {
    let typed = infer_module("drums = bd sn cp", ReplMode::Strict).unwrap();
    assert_eq!(typed.type_of("drums").to_string(), "Pattern<Sample>");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orpheus-lang --test parser duplicate_parameter_names_are_rejected`

Run: `cargo test -p orpheus-lang --test eval self_recursive_parameterized_binding_is_rejected`

Run: `cargo test -p orpheus-lang --test infer sequencing_syntax_still_parses_as_sequence`

Expected: the new error cases fail until diagnostics and guards are added.

**Step 3: Write minimal implementation**

Add:

- duplicate parameter validation in the parser or binding builder
- explicit self-reference rejection when constructing/evaluating parameterized bindings
- regression coverage ensuring juxtaposition still means sequence

**Step 4: Run tests to verify they pass**

Run: `cargo test -p orpheus-lang --test parser`

Run: `cargo test -p orpheus-lang --test eval`

Run: `cargo test -p orpheus-lang --test infer`

Expected: PASS

**Step 5: Commit**

```bash
git add crates/orpheus-lang/tests/parser.rs crates/orpheus-lang/tests/eval.rs crates/orpheus-lang/tests/infer.rs
git commit -m "test(lang): lock parameterized binding diagnostics and regressions"
```

### Task 5: Verify The Whole Language Crate

**Files:**
- No code changes expected

**Step 1: Run formatting**

Run: `cargo fmt --all`

Expected: PASS

**Step 2: Run clippy**

Run: `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`

Expected: PASS

**Step 3: Run crate tests**

Run: `cargo test -p orpheus-lang --all-targets`

Expected: PASS

**Step 4: Commit**

```bash
git add -A
git commit -m "feat(lang): add top-level parameterized bindings"
```
