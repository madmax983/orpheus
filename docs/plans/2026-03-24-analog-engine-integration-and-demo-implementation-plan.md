# Analog Engine Integration And Demo Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Integrate analog synth tokens into the existing engine/sample-pattern path and add a showcase `.ode` file that exercises the language, routing, and shared FX work together.

**Architecture:** Keep routing on `TrackSource::SamplePattern(...)`, extend scheduled events to carry span duration and synth-specific controls, render `saw`/`pulse`/`tri`/`noise` through `AnalogVoice`, and ship one TUI-loadable demo patch. Avoid new track source types or a language-level `synth(...)` surface in this slice.

**Tech Stack:** Rust 2024, `orpheus-dsp`, `orpheus-lang`, existing synth primitives/wrapper, current session/TUI path, `cargo fmt`, `cargo clippy`, `cargo test`, Verus for existing proof spine.

---

### Task 1: Write Red Engine Tests For Analog Tokens

**Files:**
- Modify: `crates/orpheus-dsp/tests/engine_commands.rs`
- Modify: `crates/orpheus-dsp/tests/offline_render.rs`

**Step 1: Add a failing live-engine audibility test**

Add a focused engine test proving a routed `saw` event renders non-silent output.

Coverage should assert:

- routing snapshot loads successfully
- a `saw` token renders audible output
- output is finite

**Step 2: Add a failing span-duration test**

Add a test showing a long synth event sustains across its scheduled span instead
of behaving like a one-shot sample trigger.

**Step 3: Add a failing shared-FX synth routing test**

Add a small test proving a synth-routed track still feeds shared delay or reverb
through the existing bus host path.

**Step 4: Verify red**

Run:

- `cargo test -p orpheus-dsp --test engine_commands analog_`
- `cargo test -p orpheus-dsp --test offline_render analog_`

Expected: FAIL because the engine still treats everything as one-shot sample or
drum fallback playback.

### Task 2: Extend Trigger And Scheduler State For Span-Aware Synth Events

**Files:**
- Modify: `crates/orpheus-dsp/src/command.rs`
- Modify: `crates/orpheus-dsp/src/scheduler.rs`

**Step 1: Add synth control fields to the trigger payload**

Extend the current trigger representation with synth-only control data:

- cutoff
- resonance
- drive
- pulse width

Keep the existing sample controls intact.

**Step 2: Add span/duration information to scheduled triggers**

Change the scheduler so scheduled events carry:

- start frame
- duration frames
- existing trigger payload

Use the pattern event span instead of only the start boundary.

**Step 3: Preserve existing sample scheduling behavior**

Sample tokens should still schedule successfully, just with more information
available than before.

**Step 4: Make the new scheduler-focused tests pass**

Run the focused engine/offline tests that previously failed.

### Task 3: Add Analog Voice Rendering To The Existing Engine Path

**Files:**
- Modify: `crates/orpheus-dsp/src/voice.rs`
- Modify: `crates/orpheus-dsp/src/engine.rs`
- Modify: `crates/orpheus-dsp/src/offline.rs`

**Step 1: Extend the voice model**

Add analog voice fallback variants for:

- `saw`
- `pulse`
- `tri`
- `noise`

Thread the span duration and synth control values into active voice creation.

**Step 2: Reuse `AnalogVoice` for render-time synthesis**

Have analog active voices own/use the wrapper and render:

- source
- ladder
- saturation
- gain

Apply a small edge envelope based on event duration to avoid clicks.

**Step 3: Preserve existing sample and drum behavior**

Do not regress:

- sample playback
- sample bank resolution
- drum fallbacks

**Step 4: Make focused live/offline analog tests green**

Run:

- `cargo test -p orpheus-dsp --test engine_commands analog_`
- `cargo test -p orpheus-dsp --test offline_render analog_`

### Task 4: Add Language Builtins And Event Fields For Synth Controls

**Files:**
- Modify: `crates/orpheus-lang/src/builtins.rs`
- Modify: `crates/orpheus-lang/src/value.rs`
- Modify: `crates/orpheus-lang/src/eval.rs`
- Modify: `crates/orpheus-lang/src/types/env.rs`
- Modify: `crates/orpheus-lang/src/types/infer.rs`
- Modify: `crates/orpheus-lang/src/mixer.rs`
- Modify: `crates/orpheus-lang/src/lib.rs`

**Step 1: Add source atoms**

Add `saw`, `pulse`, `tri`, and `noise` as playable sample-pattern-like atoms.

**Step 2: Add synth transforms**

Add:

- `cutoff`
- `res`
- `drive`
- `pw`

in the same pattern-transform style as `gain`, `pan`, `rate`, and `lpf`.

**Step 3: Extend sample-pattern event data**

Carry synth-only fields through evaluation and into mixer compilation so routed
tracks produce trigger payloads with the correct synth controls.

**Step 4: Add red/green language tests**

Add infer/eval/session tests proving:

- synth atoms infer as `Pattern<Sample>`
- synth transforms compose in direct-call and pipe forms
- track binding and session playback accept synth-backed patterns

### Task 5: Add The Showcase `.ode` And Load Path Coverage

**Files:**
- Create: `examples/analog_showcase.ode`
- Modify: `crates/orpheus-lang/src/session.rs`

**Step 1: Write one real demo patch**

The file should include:

- sample drums
- synth bass
- synth lead
- at least one rhythm-algebra construct
- at least one voicing/timing construct

Keep it musically coherent, not just a feature collage.

**Step 2: Add a session/open test**

Add a test that opens the showcase file and verifies:

- bindings load
- at least one synth binding is present
- the session can route/play it without error

**Step 3: Verify the demo is actually usable**

The final pass should include manually loading it in the TUI/session path.

### Task 6: Update Design Notes And Surface Docs

**Files:**
- Modify: `docs/plans/analog_modeled_synthesis.md`
- Modify: `docs/plans/2026-03-24-analog-engine-integration-and-demo-design.md`
- Modify: `docs/plans/2026-03-24-analog-engine-integration-and-demo-implementation-plan.md`

**Step 1: Update the roadmap note**

Point the analog synthesis roadmap from the wrapper slice to the engine
integration/demo slice.

**Step 2: Keep docs aligned with any small implementation reality changes**

If naming or touched files shift slightly during implementation, update the docs
before calling the work done.

### Task 7: Full Verification And Sludge Scan

**Step 1: Run verification**

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-dsp -p orpheus-lang --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-dsp --all-targets`
- `cargo test -p orpheus-lang --all-targets`
- `C:\Users\markm\verus\verus.exe proofs\analog_primitives.rs`

**Step 2: Scan touched areas**

Run:

- `rg -n "TODO|FIXME|Stub:" crates/orpheus-dsp/src/voice.rs crates/orpheus-dsp/src/engine.rs crates/orpheus-dsp/src/scheduler.rs crates/orpheus-dsp/src/command.rs crates/orpheus-lang/src/builtins.rs crates/orpheus-lang/src/value.rs crates/orpheus-lang/src/eval.rs crates/orpheus-lang/src/mixer.rs crates/orpheus-lang/src/session.rs examples/analog_showcase.ode docs/plans/2026-03-24-analog-engine-integration-and-demo-design.md docs/plans/2026-03-24-analog-engine-integration-and-demo-implementation-plan.md`

**Step 3: Review final shape**

Confirm:

- routing architecture stayed intact
- synth tokens still travel through the existing mixer/bus path
- event spans now matter for synth duration
- the showcase `.ode` is actually playable

## Notes

- Do not introduce a new `TrackSource` variant in this slice.
- Do not add a `synth(...)` language surface in this slice.
- Do not add full ADSR/envelope language support in this slice.
- If the trigger payload starts wanting a total rename from `SampleTrigger` to a broader voice-oriented type, that can be a follow-on cleanup after behavior is proven.
