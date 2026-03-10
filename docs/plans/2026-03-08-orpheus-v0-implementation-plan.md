# Orpheus V0 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build an Orpheus v0 that can parse cycle patterns, evaluate deterministic pattern queries, produce real audio from a minimal REPL, and compile strict `.ode` files with useful type errors.

**Architecture:** Convert the repo into a workspace with three library crates that match the design doc: `orpheus-pattern` for temporal semantics, `orpheus-dsp` for audio rendering and scheduling, and `orpheus-lang` for parsing, evaluation, typing, and REPL/TUI support. Deliver the system as vertical slices: verified time core first, then cycle patterns, then parser/evaluator, then audio, then strict typing and file loading. Keep proofs concentrated around time/event invariants and deterministic query behavior; keep the audio thread allocation-free and lock-free.

**Tech Stack:** Rust 2024, Verus, pest, anyhow, thiserror, cpal, ratatui, crossterm, rtrb, hound, num-rational, proptest, assert_cmd, predicates, insta

---

## Recommended Delivery Strategy

Use a **vertical-slice-first** plan.

- Recommended: bootstrap the workspace, then make `bd sn cp sn` audible end-to-end as fast as possible.
- Not recommended: build Hindley-Milner first. You will get a beautiful type checker and no sound.
- Also not recommended: build the full DSP graph first. That is how you end up benchmarking silence.

## Assumptions

- V0 is audio-only. MIDI, OSC, Ableton Link, and collaboration features stay out of scope.
- `bd`, `sn`, and `cp` start as built-in synthesized voices, not external samples. WAV loading lands after the first end-to-end slice is stable.
- `num-rational::Ratio<i64>` is acceptable for v0. Replace it only if profiling shows it is a real problem.
- `rtrb` is the lock-free queue for v0. Hand-rolling a queue can wait until the rest of the engine exists.
- The root crate remains the executable entrypoint. The three design-layer crates become library members under `crates/`.

## Target Repository Layout

```text
Cargo.toml
CLAUDE.md
docs/
  adr/
    0001-workspace-layout.md
    0002-time-model-and-scheduling.md
  design/
    orpheus_design.md
  plans/
    2026-03-08-orpheus-v0-implementation-plan.md
proofs/
  pattern_time.rs
  cycle_query.rs
crates/
  orpheus-pattern/
    Cargo.toml
    src/
      lib.rs
      rational.rs
      time.rs
      event.rs
      cycle.rs
      transform.rs
      stream.rs
    tests/
      workspace_smoke.rs
      time_span.rs
      cycle_pattern.rs
      transform.rs
  orpheus-dsp/
    Cargo.toml
    src/
      lib.rs
      command.rs
      scheduler.rs
      voice.rs
      sample.rs
      engine.rs
      graph.rs
    tests/
      scheduler.rs
      engine_commands.rs
  orpheus-lang/
    Cargo.toml
    src/
      lib.rs
      ast.rs
      value.rs
      builtins.rs
      parser.rs
      eval.rs
      diagnostics.rs
      loader.rs
      repl.rs
      tui.rs
      grammar/
        orpheus.pest
      types/
        mod.rs
        infer.rs
        env.rs
    tests/
      parser.rs
      eval.rs
      infer.rs
      loader.rs
src/
  main.rs
tests/
  fixtures/
    drums.ode
    song.ode
    missing_name.ode
    kick.wav
  repl_smoke.rs
  ode_smoke.rs
```

## Quality Gates

- Every runtime behavior change starts with a failing Rust test.
- Every critical temporal invariant gets a Verus spec before the runtime implementation is considered done.
- Run for each completed task:
  - `cargo fmt --all`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - targeted `cargo test ...`
- Before declaring v0 done, also run:
  - `cargo test --all-targets --all-features`
  - `C:\Users\markm\verus\verus.exe proofs/pattern_time.rs`
  - `C:\Users\markm\verus\verus.exe proofs/cycle_query.rs`

## Task 1: Bootstrap The Workspace

**Files:**
- Create: `CLAUDE.md`
- Create: `docs/adr/0001-workspace-layout.md`
- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Create: `crates/orpheus-pattern/Cargo.toml`
- Create: `crates/orpheus-pattern/src/lib.rs`
- Create: `crates/orpheus-dsp/Cargo.toml`
- Create: `crates/orpheus-dsp/src/lib.rs`
- Create: `crates/orpheus-lang/Cargo.toml`
- Create: `crates/orpheus-lang/src/lib.rs`
- Create: `crates/orpheus-pattern/tests/workspace_smoke.rs`

**Step 1: Write the failing test**

```rust
use orpheus_dsp::EngineHandle;
use orpheus_lang::ReplMode;
use orpheus_pattern::TimeSpan;

#[test]
fn workspace_bootstrap_links_crates() {
    let _mode = ReplMode::Loose;
    let span = TimeSpan::unit();
    let _engine = EngineHandle::stub();
    assert_eq!(span.start_numer(), 0);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p orpheus-pattern workspace_bootstrap_links_crates --test workspace_smoke -- --exact`
Expected: FAIL with unresolved crate imports and missing public types.

**Step 3: Write minimal implementation**

```rust
// crates/orpheus-pattern/src/lib.rs
pub struct TimeSpan;

impl TimeSpan {
    pub fn unit() -> Self { Self }
    pub fn start_numer(&self) -> i64 { 0 }
}

// crates/orpheus-dsp/src/lib.rs
pub struct EngineHandle;

impl EngineHandle {
    pub fn stub() -> Self { Self }
}

// crates/orpheus-lang/src/lib.rs
pub enum ReplMode {
    Loose,
    Strict,
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p orpheus-pattern workspace_bootstrap_links_crates --test workspace_smoke -- --exact`
Expected: PASS

**Step 5: Commit**

```bash
git add Cargo.toml CLAUDE.md docs/adr/0001-workspace-layout.md src/main.rs crates tests
git commit -m "build: bootstrap orpheus workspace"
```

## Task 2: Specify And Prove The Time Model

**Files:**
- Create: `docs/adr/0002-time-model-and-scheduling.md`
- Create: `proofs/pattern_time.rs`
- Modify: `crates/orpheus-pattern/src/lib.rs`
- Create: `crates/orpheus-pattern/src/rational.rs`
- Create: `crates/orpheus-pattern/src/time.rs`
- Create: `crates/orpheus-pattern/src/event.rs`
- Create: `crates/orpheus-pattern/tests/time_span.rs`

**Step 1: Write the failing tests**

```rust
use orpheus_pattern::{Rational, TimeSpan};

#[test]
fn rational_thirds_sum_exactly_to_one() {
    let third = Rational::new(1, 3).unwrap();
    assert_eq!(third.clone() + third.clone() + third, Rational::one());
}

#[test]
fn timespan_new_rejects_end_before_start() {
    let start = Rational::new(2, 1).unwrap();
    let end = Rational::new(1, 1).unwrap();
    assert!(TimeSpan::new(start, end).is_err());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orpheus-pattern --test time_span`
Expected: FAIL with missing `Rational`, `TimeSpan::new`, and error types.

**Step 3: Write the Verus spec first**

```rust
// proofs/pattern_time.rs
spec fn valid_span(start: int, end: int) -> bool {
    start <= end
}

proof fn span_split_preserves_bounds(start: int, mid: int, end: int)
    requires start <= mid <= end
    ensures valid_span(start, mid) && valid_span(mid, end)
{
}
```

**Step 4: Implement the runtime types**

```rust
// crates/orpheus-pattern/src/time.rs
pub struct TimeSpan {
    pub start: Rational,
    pub end: Rational,
}

impl TimeSpan {
    pub fn new(start: Rational, end: Rational) -> Result<Self, PatternError> {
        if start > end {
            return Err(PatternError::InvalidSpan { start, end });
        }
        Ok(Self { start, end })
    }

    pub fn unit() -> Self {
        Self::new(Rational::zero(), Rational::one()).unwrap()
    }
}
```

**Step 5: Run proofs and tests**

Run: `C:\Users\markm\verus\verus.exe proofs/pattern_time.rs`
Expected: verification succeeds

Run: `cargo test -p orpheus-pattern --test time_span`
Expected: PASS

**Step 6: Commit**

```bash
git add docs/adr/0002-time-model-and-scheduling.md proofs/pattern_time.rs crates/orpheus-pattern
git commit -m "feat: add verified rational time core"
```

## Task 3: Implement Cycle Patterns And Query Semantics

**Files:**
- Modify: `crates/orpheus-pattern/src/lib.rs`
- Create: `crates/orpheus-pattern/src/cycle.rs`
- Create: `proofs/cycle_query.rs`
- Create: `crates/orpheus-pattern/tests/cycle_pattern.rs`

**Step 1: Write the failing tests**

```rust
use orpheus_pattern::{CyclePattern, Pattern, PatternNode, Rational, TimeSpan};

#[test]
fn cycle_pattern_divides_unit_span_evenly() {
    let pattern = CyclePattern::from_nodes(vec![
        PatternNode::atom("bd"),
        PatternNode::atom("sn"),
        PatternNode::atom("cp"),
    ]);

    let events = pattern.query(TimeSpan::unit());
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].part.start, Rational::zero());
    assert_eq!(events[1].part.start, Rational::new(1, 3).unwrap());
}

#[test]
fn cycle_pattern_repeats_across_cycle_boundaries() {
    let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("bd")]);
    let span = TimeSpan::new(Rational::one(), Rational::new(2, 1).unwrap()).unwrap();
    let events = pattern.query(span);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].part.start, Rational::one());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orpheus-pattern --test cycle_pattern`
Expected: FAIL with missing `Pattern`, `PatternNode`, and `CyclePattern`.

**Step 3: Write the Verus proof for deterministic subdivision**

```rust
// proofs/cycle_query.rs
proof fn equal_subdivision_covers_whole_cycle(len: nat)
    requires len > 0
    ensures true
{
}
```

**Step 4: Implement the minimal cycle runtime**

```rust
pub trait Pattern<T>: Send + Sync {
    fn query(&self, span: TimeSpan) -> Vec<Event<T>>;
}

pub enum PatternNode<T> {
    Atom(T),
    Rest,
    Group(Vec<PatternNode<T>>),
}

pub struct CyclePattern<T> {
    nodes: Vec<PatternNode<T>>,
}
```

Implement query behavior in this order:
- flat subdivision
- nested groups
- cycle repetition
- `whole` vs `part` clipping for boundary-crossing events

**Step 5: Run proofs and tests**

Run: `C:\Users\markm\verus\verus.exe proofs/cycle_query.rs`
Expected: verification succeeds

Run: `cargo test -p orpheus-pattern --test cycle_pattern`
Expected: PASS

**Step 6: Add one property test**

Add a `proptest!` case asserting queried event parts never escape the requested span.

Run: `cargo test -p orpheus-pattern --test cycle_pattern queried_event_parts_stay_within_requested_span -- --exact`
Expected: PASS

**Step 7: Commit**

```bash
git add proofs/cycle_query.rs crates/orpheus-pattern
git commit -m "feat: implement cycle pattern query engine"
```

## Task 4: Parse Phase 1 Syntax Into An AST

**Files:**
- Modify: `crates/orpheus-lang/src/lib.rs`
- Create: `crates/orpheus-lang/src/ast.rs`
- Create: `crates/orpheus-lang/src/parser.rs`
- Create: `crates/orpheus-lang/src/diagnostics.rs`
- Create: `crates/orpheus-lang/src/grammar/orpheus.pest`
- Create: `crates/orpheus-lang/tests/parser.rs`

**Step 1: Write the failing tests**

```rust
use orpheus_lang::{parse_module, Expr, Stmt};

#[test]
fn parses_juxtaposition_as_sequence() {
    let module = parse_module("drums = bd sn cp").unwrap();
    match &module.statements[0] {
        Stmt::Binding { expr, .. } => assert!(matches!(expr, Expr::Seq(_))),
        other => panic!("unexpected AST: {other:?}"),
    }
}

#[test]
fn parses_pipe_as_left_associative() {
    let module = parse_module("drums = bd sn |> fast(2) |> rev").unwrap();
    let rendered = format!("{:#?}", module.statements[0]);
    assert!(rendered.contains("Pipe"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orpheus-lang --test parser`
Expected: FAIL with missing parser API and AST types.

**Step 3: Implement the grammar and AST**

```rust
pub enum Expr {
    Seq(Vec<Expr>),
    Stack(Vec<Expr>),
    Pipe { lhs: Box<Expr>, rhs: Box<Expr> },
    Call { callee: Box<Expr>, args: Vec<Expr> },
    Group(Vec<Expr>),
    Ident(String),
    Rest,
    Number(f64),
}

pub enum Stmt {
    Binding { name: String, expr: Expr },
}
```

Grammar coverage for this task:
- binding
- identifier
- rest `~`
- juxtaposition
- grouping
- function call
- pipe `|>`
- `stack(...)`

**Step 4: Run tests to verify they pass**

Run: `cargo test -p orpheus-lang --test parser`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/orpheus-lang
git commit -m "feat: parse phase-one orpheus syntax"
```

## Task 5: Evaluate AST Into Runtime Patterns

**Files:**
- Modify: `crates/orpheus-lang/src/lib.rs`
- Create: `crates/orpheus-lang/src/value.rs`
- Create: `crates/orpheus-lang/src/builtins.rs`
- Create: `crates/orpheus-lang/src/eval.rs`
- Create: `crates/orpheus-lang/tests/eval.rs`

**Step 1: Write the failing tests**

```rust
use orpheus_lang::{eval_module, ReplMode, Value};

#[test]
fn evaluating_sequence_produces_sample_pattern() {
    let module = eval_module("drums = bd sn cp sn", ReplMode::Loose).unwrap();
    match module.get("drums").unwrap() {
        Value::SamplePattern(pattern) => {
            let events = pattern.query_unit();
            assert_eq!(events.len(), 4);
        }
        other => panic!("expected sample pattern, got {other:?}"),
    }
}

#[test]
fn stack_merges_parallel_layers() {
    let module = eval_module("drums = stack(bd ~, ~ sn)", ReplMode::Loose).unwrap();
    let pattern = module.get("drums").unwrap().as_sample_pattern().unwrap();
    assert_eq!(pattern.query_unit().len(), 2);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orpheus-lang --test eval`
Expected: FAIL with missing evaluator and runtime value model.

**Step 3: Implement the minimal runtime value model**

```rust
pub enum Value {
    SamplePattern(SamplePatternValue),
    NumberPattern(NumberPatternValue),
    Function(BuiltinFn),
}
```

Implement built-ins in this order:
- sample identifiers: `bd`, `sn`, `cp`, `hh`
- `stack`
- `fast`
- `slow`
- `rev`
- `gain`

Pipe desugaring rule:
- `lhs |> fast(2)` becomes `fast(2, lhs)`
- `lhs |> every(4, fast(2))` becomes `every(4, fast(2), lhs)`

**Step 4: Run tests to verify they pass**

Run: `cargo test -p orpheus-lang --test eval`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/orpheus-lang
git commit -m "feat: add evaluator and builtin pattern transforms"
```

## Task 6: Add Minimal DSP Rendering And Lock-Free Scheduling

**Files:**
- Modify: `crates/orpheus-dsp/src/lib.rs`
- Create: `crates/orpheus-dsp/src/command.rs`
- Create: `crates/orpheus-dsp/src/scheduler.rs`
- Create: `crates/orpheus-dsp/src/voice.rs`
- Create: `crates/orpheus-dsp/src/engine.rs`
- Create: `crates/orpheus-dsp/tests/scheduler.rs`
- Create: `crates/orpheus-dsp/tests/engine_commands.rs`

**Step 1: Write the failing tests**

```rust
use orpheus_dsp::{EngineCommand, Scheduler};

#[test]
fn scheduler_emits_due_events_in_order() {
    let mut scheduler = Scheduler::new_for_test();
    scheduler.push_test_event(0, "bd");
    scheduler.push_test_event(32, "sn");
    let due = scheduler.drain_due_events(32);
    assert_eq!(due, vec!["bd", "sn"]);
}

#[test]
fn pattern_swap_is_deferred_until_cycle_boundary() {
    let mut engine = orpheus_dsp::EngineHandle::stub();
    engine.enqueue(EngineCommand::SwapPattern("verse".into()));
    assert!(!engine.swap_applied_before_boundary());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orpheus-dsp --tests`
Expected: FAIL with missing scheduler and command types.

**Step 3: Implement the minimal DSP engine**

```rust
pub enum EngineCommand {
    SwapPattern(String),
    SetTempo(f32),
}

pub struct Scheduler { /* sample-clock based queue */ }

pub enum VoiceKind {
    KickLike,
    SnareLike,
    ClapLike,
    HiHatLike,
}
```

Implement in this order:
- scheduler that converts cycle events into sample-clock triggers
- built-in voices for `bd`, `sn`, `cp`, `hh`
- `rtrb` command queue between UI/REPL thread and audio thread
- cpal engine that renders stereo buffers
- boundary-safe pattern swap at cycle boundaries

**Step 4: Run tests to verify they pass**

Run: `cargo test -p orpheus-dsp --tests`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/orpheus-dsp
git commit -m "feat: add minimal audio engine and scheduler"
```

## Task 7: Wire A Minimal REPL End To End

**Files:**
- Modify: `src/main.rs`
- Create: `crates/orpheus-lang/src/repl.rs`
- Create: `tests/repl_smoke.rs`

**Step 1: Write the failing test**

```rust
use assert_cmd::Command;

#[test]
fn repl_accepts_pattern_and_reports_success() {
    let mut cmd = Command::cargo_bin("orpheus").unwrap();
    cmd.write_stdin("drums = bd sn cp sn\n:quit\n")
        .assert()
        .success()
        .stdout(predicates::str::contains("[Pattern<Sample>] ok"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test repl_accepts_pattern_and_reports_success --test repl_smoke -- --exact`
Expected: FAIL because the binary still prints `Hello, world!`.

**Step 3: Implement the minimal REPL**

```rust
fn main() -> anyhow::Result<()> {
    orpheus_lang::repl::run_stdio()
}
```

Required REPL behavior:
- read one line at a time
- ignore blank lines
- exit on `:quit`
- evaluate bindings in loose mode
- print `[Pattern<Sample>] ok` or a human-readable error
- send active pattern updates to `orpheus-dsp`

**Step 4: Run test to verify it passes**

Run: `cargo test repl_accepts_pattern_and_reports_success --test repl_smoke -- --exact`
Expected: PASS

**Step 5: Manual happy-path check**

Run: `cargo run`
Then enter:

```text
drums = bd sn cp sn
:quit
```

Expected: the REPL acknowledges the pattern and audio output starts before quit.

**Step 6: Commit**

```bash
git add src/main.rs crates/orpheus-lang/src/repl.rs tests/repl_smoke.rs
git commit -m "feat: wire minimal orpheus repl"
```

## Task 8: Add Dual-Mode Hindley-Milner Type Inference

**Files:**
- Modify: `crates/orpheus-lang/src/lib.rs`
- Create: `crates/orpheus-lang/src/types/mod.rs`
- Create: `crates/orpheus-lang/src/types/infer.rs`
- Create: `crates/orpheus-lang/src/types/env.rs`
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/diagnostics.rs`
- Create: `crates/orpheus-lang/tests/infer.rs`

**Step 1: Write the failing tests**

```rust
use orpheus_lang::{infer_module, ReplMode};

#[test]
fn loose_mode_coerces_number_literals_to_number_patterns() {
    let typed = infer_module("cutoff = 400 800 1200", ReplMode::Loose).unwrap();
    assert_eq!(typed.type_of("cutoff").to_string(), "Pattern<Number>");
}

#[test]
fn strict_mode_rejects_mixed_stack_types() {
    let err = infer_module("drums = stack(bd sn, C4 E4)", ReplMode::Strict).unwrap_err();
    assert!(err.to_string().contains("Pattern<Sample>"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orpheus-lang --test infer`
Expected: FAIL with missing type inference entrypoints.

**Step 3: Implement the minimal inference engine**

```rust
pub enum Type {
    Pattern(Box<Type>),
    Sample,
    Note,
    Number,
    Duration,
    String,
    Function(Vec<Type>, Box<Type>),
    Var(TypeVarId),
    Unit,
}
```

Implement in this order:
- type environment for built-ins
- unification with occurs check
- loose-mode coercion table
- strict-mode error diagnostics with source spans
- typed evaluator hook so REPL prints inferred types

**Step 4: Run tests to verify they pass**

Run: `cargo test -p orpheus-lang --test infer`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/orpheus-lang
git commit -m "feat: add dual-mode type inference"
```

## Task 9: Load Strict `.ode` Files And Imports

**Files:**
- Modify: `crates/orpheus-lang/src/lib.rs`
- Create: `crates/orpheus-lang/src/loader.rs`
- Create: `crates/orpheus-lang/tests/loader.rs`
- Create: `tests/ode_smoke.rs`
- Create: `tests/fixtures/drums.ode`
- Create: `tests/fixtures/song.ode`
- Create: `tests/fixtures/missing_name.ode`

**Step 1: Write the failing tests**

```rust
use orpheus_lang::load_file_strict;

#[test]
fn loader_resolves_use_imports() {
    let module = load_file_strict("tests/fixtures/song.ode").unwrap();
    assert!(module.contains_key("song"));
}

#[test]
fn loader_reports_missing_names_as_errors() {
    let err = load_file_strict("tests/fixtures/missing_name.ode").unwrap_err();
    assert!(err.to_string().contains("unresolved name"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orpheus-lang --test loader`
Expected: FAIL with missing file loader and import resolution.

**Step 3: Implement the loader**

```rust
pub fn load_file_strict(path: impl AsRef<Path>) -> Result<TypedModule, Diagnostic> {
    // parse file, resolve imports, infer in strict mode, return typed bindings
}
```

Required behavior:
- support `use "file.ode" (name1, name2)`
- resolve imports relative to the importing file
- reject unresolved names in strict mode
- preserve source spans across imported files for diagnostics

**Step 4: Run tests to verify they pass**

Run: `cargo test -p orpheus-lang --test loader`
Expected: PASS

Run: `cargo test ode_smoke_compiles_song_file --test ode_smoke -- --exact`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/orpheus-lang tests
git commit -m "feat: load strict ode modules with imports"
```

## Task 10: Add WAV Fixture Loader Support

**Files:**
- Modify: `crates/orpheus-dsp/src/lib.rs`
- Create: `crates/orpheus-dsp/src/sample.rs`
- Modify: `Cargo.toml`
- Modify: `crates/orpheus-dsp/Cargo.toml`
- Create: `crates/orpheus-dsp/tests/sample.rs`
- Create: `tests/fixtures/kick.wav`

**Step 1: Write the failing test**

```rust
#[test]
fn wav_loader_decodes_mono_f32_samples() {
    let sample = orpheus_dsp::load_wav_for_test("tests/fixtures/kick.wav").unwrap();
    assert!(!sample.frames.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p orpheus-dsp wav_loader_decodes_mono_f32_samples -- --exact`
Expected: FAIL with missing WAV loading API.

**Step 3: Implement minimal WAV support**

Use `hound` first. Accept mono/stereo PCM WAV only. Normalize to `f32`.

Also add one boundary test for rejecting unsupported channel counts.

**Step 4: Run test to verify it passes**

Run: `cargo test -p orpheus-dsp wav_loader_decodes_mono_f32_samples -- --exact`
Expected: PASS

**Step 5: Commit**

```bash
git add Cargo.toml crates/orpheus-dsp tests/fixtures/kick.wav
git commit -m "feat: add wav sample loading"
```

## Task 10A: Wire WAV-Backed Live Playback For Built-In Drum Tokens

**Files:**
- Create: `crates/orpheus-dsp/src/sample_bank.rs`
- Modify: `crates/orpheus-dsp/src/engine.rs`
- Modify: `crates/orpheus-dsp/src/lib.rs`
- Modify: `crates/orpheus-dsp/src/sample.rs`
- Modify: `crates/orpheus-dsp/src/voice.rs`
- Modify: `crates/orpheus-dsp/tests/engine_commands.rs`
- Create: `crates/orpheus-dsp/assets/kick.wav`
- Create: `crates/orpheus-dsp/assets/snare.wav`
- Create: `crates/orpheus-dsp/assets/clap.wav`
- Create: `crates/orpheus-dsp/assets/hihat.wav`

**Step 1: Write the failing engine test**

```rust
#[test]
fn built_in_bd_trigger_prefers_embedded_wav_frames() {
    let mut engine = EngineHandle::new_for_test();
    let sample = orpheus_dsp::load_builtin_sample_for_test("bd").unwrap();

    engine.schedule_test_trigger(0, "bd");
    let rendered = engine.render_test_block(8);

    let expected = vec![
        sample.frames[0],
        sample.frames[0],
        sample.frames[1],
        sample.frames[1],
        sample.frames[2],
        sample.frames[2],
        sample.frames[3],
        sample.frames[3],
    ];

    assert_eq!(rendered, expected);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p orpheus-dsp built_in_bd_trigger_prefers_embedded_wav_frames --test engine_commands -- --exact`
Expected: FAIL because the engine still renders the synthesized fallback voice.

**Step 3: Implement the minimal live sample bank**

Use this design:
- embed tiny default one-shot WAV assets under `crates/orpheus-dsp/assets/`
- decode them once during engine initialization into `Arc<[f32]>`
- keep `VoiceKind` as the token key
- add a `SampleBank` owned by the render engine
- extend `ActiveVoice` so it can render either:
  - a synthesized fallback voice, or
  - a sample-backed voice with a fractional frame cursor
- support mono and stereo sources by averaging stereo frames to mono before mixing
- support output sample rates different from the source sample rate with a simple fractional step (`source_rate / output_rate`)
- if a built-in sample is unavailable or invalid, fall back to the existing synthesized voice instead of muting the event

Expose one narrow test-only helper:

```rust
pub fn load_builtin_sample_for_test(name: &str) -> Result<DecodedSample, SampleError>;
```

Keep language/token plumbing unchanged:
- `bd`, `sn`, `cp`, `hh` stay the public identifiers
- no `orpheus-lang` changes are required for this task

**Step 4: Run tests to verify they pass**

Run: `cargo test -p orpheus-dsp --test engine_commands`
Expected: PASS

Run: `cargo test -p orpheus-dsp --test sample`
Expected: PASS

**Step 5: Run the full verification sweep**

Run: `cargo fmt --all -- --check`
Expected: PASS

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: PASS

Run: `cargo test --workspace --all-targets --all-features`
Expected: PASS

**Step 6: Commit**

```bash
git add Cargo.toml crates/orpheus-dsp
git commit -m "feat: wire wav-backed live drum playback"
```

## Task 11: Add Ratatui Session UI

**Files:**
- Create: `crates/orpheus-lang/src/tui.rs`
- Modify: `crates/orpheus-lang/src/repl.rs`
- Modify: `src/main.rs`
- Create: `tests/tui_smoke.rs`

**Step 1: Write the failing smoke test**

```rust
#[test]
fn tui_boots_and_renders_initial_frame() {
    // Render a single frame against ratatui::backend::TestBackend
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test tui_boots_and_renders_initial_frame --test tui_smoke -- --exact`
Expected: FAIL because there is no TUI module.

**Step 3: Implement the minimal TUI**

First frame only:
- left pane: current bindings
- bottom pane: REPL input/output
- right pane: transport stub

Add oscilloscope/spectrum widgets only after the frame structure is stable.

**Step 4: Run test to verify it passes**

Run: `cargo test tui_boots_and_renders_initial_frame --test tui_smoke -- --exact`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/orpheus-lang src/main.rs tests/tui_smoke.rs
git commit -m "feat: add initial orpheus tui shell"
```

## Task 12: Add Event Streams, Meter Metadata, And Song Rendering

**Files:**
- Modify: `crates/orpheus-pattern/src/lib.rs`
- Create: `crates/orpheus-pattern/src/stream.rs`
- Modify: `crates/orpheus-lang/src/ast.rs`
- Modify: `crates/orpheus-lang/src/parser.rs`
- Modify: `crates/orpheus-lang/src/eval.rs`
- Create: `crates/orpheus-pattern/tests/stream.rs`
- Create: `tests/render_smoke.rs`

**Step 1: Write the failing tests**

```rust
#[test]
fn stream_query_returns_events_in_explicit_time_order() {
    // assert stream(at(0, ...), at(5/2, ...)) queries correctly
}

#[test]
fn meter_translates_beats_into_cycle_relative_time() {
    // assert beat(2) in 4/4 maps to cycle offset 1/2
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p orpheus-pattern stream_query_returns_events_in_explicit_time_order -- --exact`
Expected: FAIL with missing event stream implementation.

**Step 3: Implement the escape hatch**

Implement in this order:
- `EventStream<T>`
- `stream(...)` and `at(...)`
- `meter(n, d)`
- `seq_sections(...)`
- offline WAV render path for exporting a song

**Step 4: Run tests to verify they pass**

Run: `cargo test -p orpheus-pattern --test stream`
Expected: PASS

Run: `cargo test render_smoke_exports_song_wav --test render_smoke -- --exact`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/orpheus-pattern crates/orpheus-lang tests
git commit -m "feat: add event streams meter and offline render"
```

## Post-V0 Backlog

Do not start these until Tasks 1-9 are stable and boring:

- Faust-style DSP graph combinators in `crates/orpheus-dsp/src/graph.rs`
- PolyBLEP oscillators and ladder filter
- richer effects chain: delay, chorus, reverb, compression
- tracker-style read-only visualization
- sample hot-reload and sample library scanning
- REPL history persistence
- recording to FLAC
- performance-mode keybindings for live sets

## Verification Checklist Before Calling V0 Done

- `bd sn cp sn` is audible from the REPL.
- REPL prints inferred types in loose mode.
- strict `.ode` compilation rejects mixed-pattern stacks.
- all targeted tests from Tasks 1-9 pass.
- `cargo test --all-targets --all-features` passes.
- `cargo clippy --all-targets --all-features -- -D warnings` passes.
- `C:\Users\markm\verus\verus.exe proofs/pattern_time.rs` passes.
- `C:\Users\markm\verus\verus.exe proofs/cycle_query.rs` passes.
- `rg "TODO|FIXME|Stub:" crates src tests docs` returns nothing relevant to shipped behavior.

Plan complete and saved to `docs/plans/2026-03-08-orpheus-v0-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open a new session with `executing-plans`, batch execution with checkpoints

Which approach?
