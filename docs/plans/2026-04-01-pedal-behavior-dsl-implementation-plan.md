# Pedal Behavior DSL Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the first executable pedal-behavior DSL slice: `graph {}` pedal bindings, pipe-friendly pedal application via `through(...)`, source-level `:explain`, control-rate local modulation, explicit safe feedback, and mono per-voice DSP execution through the existing sample/synth pattern runtime.

**Architecture:** Keep routing on the existing `Pattern<Sample>` and `TrackSource::SamplePattern(...)` path. A pedal binding compiles off the audio thread into a validated internal pedal program; `through(...)` attaches that program to sample/synth events, and `ActiveVoice` instantiates a mono pedal processor before stereo panning/output gain. Do not depend on live-audio input for v1; inside a pedal graph, `input` means "the current voice signal," not a hardware input device.

**Tech Stack:** Rust 2024, `pest` parser, `orpheus-lang` AST/type/eval/session layers, `orpheus-dsp` graph/voice runtime, Verus in `proofs/pedal_graph.rs`, targeted `cargo test`, `cargo fmt`, `cargo clippy`

---

### Task 1: Extend The Language Surface For Pedal Graphs

**Files:**
- Modify: `crates/orpheus-lang/src/ast.rs`
- Modify: `crates/orpheus-lang/src/grammar/orpheus.pest`
- Modify: `crates/orpheus-lang/src/parser.rs`
- Test: `crates/orpheus-lang/tests/parser.rs`

**Step 1: Write the failing parser tests**

Add parser coverage for:
- `drivebox = graph { wet = input |> clip(model=silicon_hard) ; wet |> output }`
- let-bound reuse inside `graph {}`
- `mix(dry * 0.2, wet * 0.8)` with `*` and `+`
- rejection of malformed `graph {}` blocks and missing result expressions

Recommended test names:
- `pedal_graph_parses_let_bound_block`
- `pedal_graph_parses_binary_control_expressions`
- `pedal_graph_requires_result_expression`

**Step 2: Run the parser tests to verify they fail**

Run:
```powershell
cargo test -p orpheus-lang --test parser pedal_graph_parses_let_bound_block -- --exact
```

Expected: FAIL because `graph`, `{`, `}`, `+`, and `*` are not part of the current AST/grammar.

**Step 3: Add the new AST forms**

Add the minimum new surface shapes:
- `Expr::Graph { bindings, result }`
- `GraphBinding { name, expr }`
- `Expr::Binary { lhs, op, rhs }`
- `BinaryOp::{Add, Mul}`

Do not add general recursion nodes. Graph recursion remains source-level syntax that must lower through explicit `feedback(...)`.

**Step 4: Extend the grammar and parser**

Add:
- `graph { ... }` block parsing
- let-bound graph statements
- operator precedence for `*` above `+`
- preservation of existing `|>` left-associativity outside and inside graph expressions

Keep the old pattern grammar intact; this is an additive change, not a rewrite of the whole parser.

**Step 5: Re-run the parser tests**

Run:
```powershell
cargo test -p orpheus-lang --test parser pedal_graph_parses_let_bound_block -- --exact
cargo test -p orpheus-lang --test parser pedal_graph_parses_binary_control_expressions -- --exact
cargo test -p orpheus-lang --test parser pedal_graph_requires_result_expression -- --exact
```

Expected: PASS

**Step 6: Commit**

```powershell
git add crates/orpheus-lang/src/ast.rs crates/orpheus-lang/src/grammar/orpheus.pest crates/orpheus-lang/src/parser.rs crates/orpheus-lang/tests/parser.rs
git commit -m "feat: parse pedal graph blocks"
```

### Task 2: Add Pedal Types, Values, And The Proof Spine

**Files:**
- Create: `proofs/pedal_graph.rs`
- Create: `crates/orpheus-lang/src/pedal.rs`
- Modify: `crates/orpheus-lang/src/lib.rs`
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/types/mod.rs`
- Modify: `crates/orpheus-lang/src/types/infer.rs`
- Test: `crates/orpheus-lang/tests/infer.rs`

**Step 1: Write the proof skeleton first**

Create `proofs/pedal_graph.rs` with a small topology spine for the invariants that actually matter:
- feed-forward bindings admit a topological evaluation order
- every referenced local binding must be defined before lowering
- recursive structure can only enter through explicit `feedback(...)`

Do not try to prove DSP soundness. Prove the graph-shape laws only.

**Step 2: Write the failing type tests**

Add inference coverage for:
- a `graph {}` binding infers as `Pedal`
- `through(pedal_binding, sample_pattern)` returns `Pattern<Sample>`
- trying to use a pedal as a number or sample directly fails with a type error

Recommended test names:
- `pedal_graph_binding_infers_pedal_type`
- `through_applies_pedal_to_sample_pattern`
- `pedal_value_is_not_a_number_pattern`

**Step 3: Run the proof and type tests to verify they fail**

Run:
```powershell
C:\Users\markm\verus\verus.exe proofs\pedal_graph.rs
cargo test -p orpheus-lang --test infer pedal_graph_binding_infers_pedal_type -- --exact
```

Expected: FAIL because `Type::Pedal`, `Value::Pedal`, and the proof model do not exist yet.

**Step 4: Add the pedal domain model**

Create `crates/orpheus-lang/src/pedal.rs` with the language-side pedal types:
- source-level pedal graph wrapper
- validated pedal plan representation
- signal kind enum for `Audio` and `Control`
- source-level explain/format hooks

Add:
- `Type::Pedal`
- `Value::Pedal(PedalValue)`

Keep `PedalValue` small and immutable. It should own validated source-level structure and compiled-plan metadata, not mutable DSP runtime state.

**Step 5: Update inference and value plumbing**

Teach `types::infer` and `value.rs` how to:
- infer graph bindings as `Pedal`
- infer `through(...)` as `Pattern<Sample>`
- reject illegal cross-kind uses cleanly

**Step 6: Re-run the proof and type tests**

Run:
```powershell
C:\Users\markm\verus\verus.exe proofs\pedal_graph.rs
cargo test -p orpheus-lang --test infer pedal_graph_binding_infers_pedal_type -- --exact
cargo test -p orpheus-lang --test infer through_applies_pedal_to_sample_pattern -- --exact
```

Expected: PASS

**Step 7: Commit**

```powershell
git add proofs/pedal_graph.rs crates/orpheus-lang/src/pedal.rs crates/orpheus-lang/src/lib.rs crates/orpheus-lang/src/value.rs crates/orpheus-lang/src/types/mod.rs crates/orpheus-lang/src/types/infer.rs crates/orpheus-lang/tests/infer.rs
git commit -m "feat: add pedal graph types and proof spine"
```

### Task 3: Lower Pedal Graphs Into A Validated Internal Plan

**Files:**
- Modify: `crates/orpheus-lang/src/pedal.rs`
- Modify: `crates/orpheus-lang/src/eval.rs`
- Modify: `crates/orpheus-lang/src/diagnostics.rs`
- Test: `crates/orpheus-lang/tests/eval.rs`

**Step 1: Write the failing evaluation tests**

Add evaluation coverage for:
- a valid pedal graph binding producing a `Value::Pedal`
- undefined local graph names producing source-level errors
- illegal implicit cycles being rejected
- legal `feedback(...)` surviving lowering
- `output` misuse being rejected with a source-level message

Recommended test names:
- `pedal_graph_binding_evaluates_to_pedal_value`
- `pedal_graph_rejects_unbound_local_signal`
- `pedal_graph_rejects_implicit_cycle`
- `pedal_graph_accepts_explicit_feedback_node`

**Step 2: Run the tests to verify they fail**

Run:
```powershell
cargo test -p orpheus-lang --test eval pedal_graph_binding_evaluates_to_pedal_value -- --exact
```

Expected: FAIL because `Expr::Graph` is not lowered or evaluated yet.

**Step 3: Implement graph validation and lowering**

In `crates/orpheus-lang/src/pedal.rs`, add a compile pipeline that:
1. resolves graph-local names
2. classifies expressions as `Audio` or `Control`
3. validates allowed parameter modulation
4. rejects implicit cycles
5. lowers explicit `feedback(...)` into a distinct internal node
6. produces a stable internal pedal plan plus a source-level explain tree

Keep diagnostics tied to source names like `wet`, `dry`, and `output`; do not leak anonymous node IDs in normal errors.

**Step 4: Hook evaluation into the normal module evaluator**

Update `eval.rs` so top-level pedal bindings:
- evaluate once off the audio thread
- store a `Value::Pedal`
- never query as a pattern directly

**Step 5: Re-run the new evaluation tests**

Run:
```powershell
cargo test -p orpheus-lang --test eval pedal_graph_binding_evaluates_to_pedal_value -- --exact
cargo test -p orpheus-lang --test eval pedal_graph_rejects_unbound_local_signal -- --exact
cargo test -p orpheus-lang --test eval pedal_graph_rejects_implicit_cycle -- --exact
cargo test -p orpheus-lang --test eval pedal_graph_accepts_explicit_feedback_node -- --exact
```

Expected: PASS

**Step 6: Commit**

```powershell
git add crates/orpheus-lang/src/pedal.rs crates/orpheus-lang/src/eval.rs crates/orpheus-lang/src/diagnostics.rs crates/orpheus-lang/tests/eval.rs
git commit -m "feat: compile pedal graphs into validated plans"
```

### Task 4: Add Pedal Application And `:explain`

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/eval.rs`
- Modify: `crates/orpheus-lang/src/session.rs`
- Modify: `crates/orpheus-lang/src/tui.rs`
- Test: `crates/orpheus-lang/tests/eval.rs`
- Test: `crates/orpheus-lang/src/session.rs`

**Step 1: Write the failing application tests**

Add language tests for one narrow application surface:
- direct call: `through(drivebox, saw)`
- pipe form: `saw |> through(drivebox)`
- invalid target: `through(drivebox, 1.0)` should fail

Add session tests for:
- `:explain drivebox` returning a readable source-level plan
- `:explain drums` failing cleanly when the binding is not a pedal

Recommended test names:
- `through_direct_call_wraps_sample_pattern`
- `through_pipe_form_matches_direct_call`
- `session_explain_returns_pedal_plan`

**Step 2: Run the tests to verify they fail**

Run:
```powershell
cargo test -p orpheus-lang --test eval through_direct_call_wraps_sample_pattern -- --exact
cargo test -p orpheus-lang --lib session_explain_returns_pedal_plan -- --exact
```

Expected: FAIL because `through` and `:explain` do not exist.

**Step 3: Add the builtin and command surface**

Implement `through(...)` as the only v1 pedal application surface.

Why this exact cut:
- the repo has no live-audio input path yet
- `input` inside a pedal graph can therefore mean "the current voice signal"
- `through(...)` lets existing sample/synth patterns use pedals immediately

Add `:explain <binding>` to `ReplSession` and TUI help text. It should render the validated pedal plan in source terms:
- bindings
- branches
- control-rate modulators
- smoothing points
- explicit feedback nodes

**Step 4: Re-run the tests**

Run:
```powershell
cargo test -p orpheus-lang --test eval through_direct_call_wraps_sample_pattern -- --exact
cargo test -p orpheus-lang --test eval through_pipe_form_matches_direct_call -- --exact
cargo test -p orpheus-lang --lib session_explain_returns_pedal_plan -- --exact
```

Expected: PASS

**Step 5: Commit**

```powershell
git add crates/orpheus-lang/src/builtins.rs crates/orpheus-lang/src/eval.rs crates/orpheus-lang/src/session.rs crates/orpheus-lang/src/tui.rs crates/orpheus-lang/tests/eval.rs
git commit -m "feat: add pedal application and explain command"
```

### Task 5: Thread Pedal Programs Through Events And Triggers

**Files:**
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/export.rs`
- Modify: `crates/orpheus-dsp/src/command.rs`
- Modify: `crates/orpheus-dsp/src/lib.rs`
- Test: `crates/orpheus-lang/tests/eval.rs`
- Test: `crates/orpheus-dsp/tests/engine_commands.rs`

**Step 1: Write the failing plumbing tests**

Add tests proving:
- `through(...)` preserves `Pattern<Sample>` behavior while attaching a pedal program
- multiple events can share the same compiled pedal plan reference
- `sample_trigger_from_event(...)` forwards pedal data into `SampleTrigger`

Recommended test names:
- `through_preserves_sample_pattern_type_and_attaches_pedal`
- `sample_trigger_carries_pedal_program`

**Step 2: Run the tests to verify they fail**

Run:
```powershell
cargo test -p orpheus-lang --test eval through_preserves_sample_pattern_type_and_attaches_pedal -- --exact
cargo test -p orpheus-dsp --test engine_commands sample_trigger_carries_pedal_program -- --exact
```

Expected: FAIL because neither `SampleEvent` nor `SampleTrigger` can carry a pedal plan.

**Step 3: Add pedal attachment fields**

Add an optional pedal program field to:
- `SampleEvent`
- `SampleTrigger`

Use a shared immutable program reference, for example `Arc<PedalProgram>`, so:
- compilation/allocation happens off the audio thread
- event/trigger cloning stays cheap
- equality in tests still works through structural program equality

**Step 4: Forward the program through conversion layers**

Update:
- `through(...)` event mutation in `value.rs`
- `sample_trigger_from_event(...)` in `export.rs`
- any trigger copy/build helpers in `command.rs`

Do not create a separate track source type. Keep the whole slice on the existing event/voice path.

**Step 5: Re-run the plumbing tests**

Run:
```powershell
cargo test -p orpheus-lang --test eval through_preserves_sample_pattern_type_and_attaches_pedal -- --exact
cargo test -p orpheus-dsp --test engine_commands sample_trigger_carries_pedal_program -- --exact
```

Expected: PASS

**Step 6: Commit**

```powershell
git add crates/orpheus-lang/src/value.rs crates/orpheus-lang/src/export.rs crates/orpheus-dsp/src/command.rs crates/orpheus-dsp/src/lib.rs crates/orpheus-lang/tests/eval.rs crates/orpheus-dsp/tests/engine_commands.rs
git commit -m "feat: thread pedal programs through triggers"
```

### Task 6: Implement The DSP Pedal Runtime And Core Behaviors

**Files:**
- Create: `crates/orpheus-dsp/src/pedal/mod.rs`
- Create: `crates/orpheus-dsp/src/pedal/program.rs`
- Create: `crates/orpheus-dsp/src/pedal/runtime.rs`
- Modify: `crates/orpheus-dsp/src/graph/adapters.rs`
- Modify: `crates/orpheus-dsp/src/lib.rs`
- Test: `crates/orpheus-dsp/tests/pedal_runtime.rs`

**Step 1: Write the failing DSP runtime tests**

Add coverage for:
- a straight serial pedal chain producing finite non-silent output
- a `graph {}` with `mix(...)` producing the expected dry/wet blend
- model selectors producing different output fingerprints without changing topology

Recommended test names:
- `pedal_runtime_renders_serial_chain`
- `pedal_runtime_mixes_named_branches`
- `pedal_models_produce_distinct_transfer_shapes`

**Step 2: Run the tests to verify they fail**

Run:
```powershell
cargo test -p orpheus-dsp --test pedal_runtime pedal_runtime_renders_serial_chain -- --exact
```

Expected: FAIL because no pedal runtime exists.

**Step 3: Build the pedal program compiler/runtime**

Implement a dedicated mono pedal runtime that compiles the validated pedal plan into DSP execution objects.

Use the existing graph substrate where it helps, but keep the pedal entry point explicit:
- audio input node
- control nodes
- behavior nodes
- explicit feedback node

Core public behavior set for this slice:
- `buffer`
- `preamp`
- `gain`
- `clip`
- `tone`
- `filter`
- `eq`
- `level`
- `sag`
- `bias`
- `constant`
- `lfo`
- `env_follow`
- `mix`
- `feedback`

If `eq`, `sag`, or `bias` threaten the slice, implement them as thin wrappers over the same runtime primitives rather than inventing separate DSP architectures.

**Step 4: Re-run the runtime tests**

Run:
```powershell
cargo test -p orpheus-dsp --test pedal_runtime pedal_runtime_renders_serial_chain -- --exact
cargo test -p orpheus-dsp --test pedal_runtime pedal_runtime_mixes_named_branches -- --exact
cargo test -p orpheus-dsp --test pedal_runtime pedal_models_produce_distinct_transfer_shapes -- --exact
```

Expected: PASS

**Step 5: Commit**

```powershell
git add crates/orpheus-dsp/src/pedal/mod.rs crates/orpheus-dsp/src/pedal/program.rs crates/orpheus-dsp/src/pedal/runtime.rs crates/orpheus-dsp/src/graph/adapters.rs crates/orpheus-dsp/src/lib.rs crates/orpheus-dsp/tests/pedal_runtime.rs
git commit -m "feat: add pedal dsp runtime"
```

### Task 7: Integrate Pedals Into `ActiveVoice`, Including Control-Rate Modulation And Safe Feedback

**Files:**
- Modify: `crates/orpheus-dsp/src/voice.rs`
- Modify: `crates/orpheus-dsp/src/pedal/runtime.rs`
- Test: `crates/orpheus-dsp/tests/offline_render.rs`
- Test: `crates/orpheus-dsp/tests/engine_commands.rs`
- Test: `crates/orpheus-dsp/tests/pedal_runtime.rs`

**Step 1: Write the failing integration tests**

Add integration coverage for:
- a sample voice running through a pedal graph
- an analog voice running through a pedal graph
- local `lfo(...)` modulation changing a continuous parameter over time
- `feedback(...)` remaining bounded and click-safe with its required delay

Recommended test names:
- `sample_voice_renders_through_pedal_program`
- `analog_voice_renders_through_pedal_program`
- `pedal_lfo_modulates_cutoff_at_control_rate`
- `pedal_feedback_requires_explicit_delay`

**Step 2: Run the tests to verify they fail**

Run:
```powershell
cargo test -p orpheus-dsp --test offline_render sample_voice_renders_through_pedal_program -- --exact
cargo test -p orpheus-dsp --test offline_render analog_voice_renders_through_pedal_program -- --exact
```

Expected: FAIL because `ActiveVoice` does not instantiate or step pedal processors.

**Step 3: Integrate pedal processing into the voice path**

Update `voice.rs` so:
- each `ActiveVoice` optionally owns a `PedalInstance`
- pedal processing runs on the mono voice signal before stereo pan/output scaling
- control nodes update at control rate
- smoothing is applied where needed to prevent zipper noise
- explicit `feedback(...)` remains the only recursive path

Do not let arbitrary graph cycles leak into the runtime. Reject them during language-side validation.

**Step 4: Re-run the integration tests**

Run:
```powershell
cargo test -p orpheus-dsp --test offline_render sample_voice_renders_through_pedal_program -- --exact
cargo test -p orpheus-dsp --test offline_render analog_voice_renders_through_pedal_program -- --exact
cargo test -p orpheus-dsp --test pedal_runtime pedal_lfo_modulates_cutoff_at_control_rate -- --exact
```

Expected: PASS

**Step 5: Commit**

```powershell
git add crates/orpheus-dsp/src/voice.rs crates/orpheus-dsp/src/pedal/runtime.rs crates/orpheus-dsp/tests/offline_render.rs crates/orpheus-dsp/tests/engine_commands.rs crates/orpheus-dsp/tests/pedal_runtime.rs
git commit -m "feat: run pedal graphs inside active voices"
```

### Task 8: Full Verification, TODO Sweep, And Documentation Cleanup

**Files:**
- Modify as needed based on failures
- Check: `proofs/pedal_graph.rs`
- Check: `crates/orpheus-lang/src/ast.rs`
- Check: `crates/orpheus-lang/src/parser.rs`
- Check: `crates/orpheus-lang/src/pedal.rs`
- Check: `crates/orpheus-lang/src/builtins.rs`
- Check: `crates/orpheus-lang/src/eval.rs`
- Check: `crates/orpheus-lang/src/session.rs`
- Check: `crates/orpheus-lang/src/value.rs`
- Check: `crates/orpheus-lang/src/export.rs`
- Check: `crates/orpheus-dsp/src/command.rs`
- Check: `crates/orpheus-dsp/src/pedal/`
- Check: `crates/orpheus-dsp/src/voice.rs`
- Check: `docs/plans/2026-03-31-pedal-behavior-dsl-design.md`
- Check: `docs/plans/2026-04-01-pedal-behavior-dsl-implementation-plan.md`

**Step 1: Run focused verification**

Run:
```powershell
C:\Users\markm\verus\verus.exe proofs\pedal_graph.rs
cargo test -p orpheus-lang --test parser
cargo test -p orpheus-lang --test infer
cargo test -p orpheus-lang --test eval
cargo test -p orpheus-dsp --test pedal_runtime
cargo test -p orpheus-dsp --test offline_render
cargo test -p orpheus-dsp --test engine_commands
```

Expected: PASS

**Step 2: Run repository-level quality gates for the touched crates**

Run:
```powershell
cargo fmt --all
cargo clippy -p orpheus-lang -p orpheus-dsp --all-targets --all-features -- -D warnings
```

Expected: PASS

**Step 3: Sweep for fake bones**

Run:
```powershell
rg -n "TODO|FIXME|Stub:" proofs/pedal_graph.rs crates/orpheus-lang/src/pedal.rs crates/orpheus-dsp/src/pedal crates/orpheus-lang/tests crates/orpheus-dsp/tests
```

Expected: no new unfinished stubs in the affected slice

**Step 4: Review the final diff**

Run:
```powershell
git diff --stat trunk...
git diff trunk... -- docs/plans/2026-03-31-pedal-behavior-dsl-design.md docs/plans/2026-04-01-pedal-behavior-dsl-implementation-plan.md crates/orpheus-lang crates/orpheus-dsp proofs/pedal_graph.rs
```

Expected: one coherent unit of work with parser, type, runtime, DSP, proof, and explain surfaces all present

**Step 5: Commit**

```powershell
git add proofs/pedal_graph.rs crates/orpheus-lang crates/orpheus-dsp docs/plans/2026-03-31-pedal-behavior-dsl-design.md docs/plans/2026-04-01-pedal-behavior-dsl-implementation-plan.md
git commit -m "feat: add pedal behavior DSL"
```
