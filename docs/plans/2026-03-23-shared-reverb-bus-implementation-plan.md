# Shared Reverb Bus Implementation Plan

> **For Claude:** Execute this as a SPEC-PROOF-RED-GREEN-REFACTOR slice. Reuse
> the hosted-bus snapshot model already proven by delay. Do not special-case
> reverb into a second runtime/control contract.

**Goal:** Add a shared bus-local reverb host on top of the existing routing
snapshot mixer architecture and shared delay host.

**Architecture:** Extend the validated routing snapshot with
`BusEffectSpec::Reverb(ReverbSpec)`, keep mutable `ReverbState` in the render
engine, preserve unchanged reverb state across unrelated routing swaps, and
surface bus reverb control through `:bus fx <bus> reverb ...`.

**Tech Stack:** Rust 2024, `orpheus-dsp`, `orpheus-lang`, `orpheus-pattern`,
`cargo fmt`, `cargo clippy`, `cargo test`, Verus in `proofs/routing_graph.rs`.

---

### Task 1: Extend Proof And Routing Snapshot Tests For Reverb Specs

**Files:**
- Modify: `proofs/routing_graph.rs`
- Modify: `crates/orpheus-dsp/tests/routing_snapshot.rs`

**Step 1: Extend the proof note**

Keep the proof surface small and honest:

- hosted bus effects remain bus-local annotations
- adding `Reverb` must not introduce new edge kinds
- valid phase-1 routes still terminate at master

This should likely reuse the existing “hosted bus effects do not create new
edges” lemmas rather than inventing reverb-specific proof religion.

**Step 2: Add red routing snapshot tests**

Add tests such as:

```rust
#[test]
fn routing_snapshot_accepts_reverb_effect_on_named_bus() {
    let snapshot = RoutingSnapshot::builder()
        .bus("verb")
        .bus_effect_reverb("verb", 0.75, 0.35, 1.0)
        .build()
        .unwrap();
    assert!(snapshot.bus("verb").unwrap().effect().is_some());
}

#[test]
fn routing_snapshot_rejects_invalid_reverb_size() {
    let error = RoutingSnapshot::builder()
        .bus("verb")
        .bus_effect_reverb("verb", 1.5, 0.35, 1.0)
        .build()
        .unwrap_err();
    assert!(error.to_string().contains("size"));
}
```

Also cover:

- invalid `damp`
- invalid `wet`
- unknown bus
- duplicate hosted effects on one bus

**Step 3: Verify red**

Run:

- `C:\Users\markm\verus\verus.exe proofs\routing_graph.rs`
- `cargo test -p orpheus-dsp --test routing_snapshot`

Expected: routing tests fail on missing reverb builder/config support.

### Task 2: Implement Reverb Specs In The Routing Snapshot

**Files:**
- Modify: `crates/orpheus-dsp/src/routing.rs`
- Modify: `crates/orpheus-dsp/src/lib.rs`

**Step 1: Add routing descriptor types**

Add:

- `ReverbSpec`
- `BusEffectSpec::Reverb(ReverbSpec)`
- validation errors for bad `size`, `damp`, `wet`

**Step 2: Extend the builder**

Add a helper:

- `bus_effect_reverb(bus_name, size, damp, wet)`

Keep one hosted effect per bus in this slice.

**Step 3: Make routing tests green**

Run:

- `C:\Users\markm\verus\verus.exe proofs\routing_graph.rs`
- `cargo test -p orpheus-dsp --test routing_snapshot`

Expected: PASS.

### Task 3: Add Red Engine And Offline Tests For Shared Reverb

**Files:**
- Modify: `crates/orpheus-dsp/tests/engine_commands.rs`
- Modify: `crates/orpheus-dsp/tests/offline_render.rs`

**Step 1: Add red engine tests**

Add tests for:

```rust
#[test]
fn engine_renders_shared_reverb_bus_tail() {
    // One impulse into a reverb bus should produce a nonzero decaying tail.
}

#[test]
fn reverb_effect_changes_adopt_only_at_cycle_boundary() {
    // A pending reverb config should not change the current cycle mid-phrase.
}

#[test]
fn unchanged_reverb_state_survives_unrelated_send_update() {
    // Recompiling routing without changing verb config must preserve tail state.
}
```

Use deterministic impulse-like samples and assert on:

- nonzero tail after the dry hit
- relative decay
- no mid-cycle config adoption

**Step 2: Add offline/live parity red test**

Mirror the shared-delay parity pattern with a reverb-hosted snapshot and ensure
offline and live render agree on the deterministic test case.

**Step 3: Verify red**

Run:

- `cargo test -p orpheus-dsp --test engine_commands`
- `cargo test -p orpheus-dsp --test offline_render`

Expected: FAIL.

### Task 4: Implement Reverb Runtime State And Bus Processing

**Files:**
- Modify: `crates/orpheus-dsp/src/effects/mod.rs`
- Create: `crates/orpheus-dsp/src/effects/reverb.rs`
- Modify: `crates/orpheus-dsp/src/engine.rs`
- Modify: `crates/orpheus-dsp/src/offline.rs`

**Step 1: Add deterministic reverb state**

Implement a small deterministic diffuse reverb topology with:

- parallel comb filters
- serial allpass diffusion
- stereo decorrelation
- `size`, `damp`, `wet` controls
- no allocation on render

Keep the implementation modest and readable. This is a proof point, not a DSP
ego contest.

**Step 2: Extend bus effect state handling**

Add:

- `BusEffectState::Reverb(ReverbState)`
- snapshot adoption logic that preserves unchanged reverb state
- reset behavior when config changes or host is removed

**Step 3: Make engine/offline tests green**

Run:

- `cargo test -p orpheus-dsp --test engine_commands`
- `cargo test -p orpheus-dsp --test offline_render`
- `cargo test -p orpheus-dsp --all-targets`

Expected: PASS.

### Task 5: Add Red Session And TUI Tests For `:bus fx ... reverb`

**Files:**
- Modify: `crates/orpheus-lang/src/session.rs`
- Modify: `crates/orpheus-lang/src/tui.rs`

**Step 1: Add session tests**

Add tests for:

```rust
#[test]
fn bus_fx_command_attaches_shared_reverb_to_bus() {
    // :bus new verb
    // :bus fx verb reverb size=0.75 damp=0.35 wet=1.0
}
```

Also cover:

- clearing with `:bus fx verb none`
- unknown bus
- invalid `size`
- invalid `damp`
- invalid `wet`

**Step 2: Add TUI tests**

Prove:

- transport pane shows hosted reverb summaries
- pending routing is visible after reverb FX changes
- command completion/help text reflects the bus FX surface

**Step 3: Verify red**

Run:

- `cargo test -p orpheus-lang bus_fx`
- `cargo test -p orpheus-lang transport_pane_shows_`

Expected: FAIL on missing reverb surface.

### Task 6: Implement Mixer, Session, And TUI Reverb Surface

**Files:**
- Modify: `crates/orpheus-lang/src/mixer.rs`
- Modify: `crates/orpheus-lang/src/session.rs`
- Modify: `crates/orpheus-lang/src/tui.rs`

**Step 1: Extend mixer bus effect model**

Add reverb to the mixer bus effect enum/state and compile it into the DSP
routing snapshot.

**Step 2: Add `:bus fx ... reverb` parsing**

Support:

- `:bus fx <bus> reverb size=<f> damp=<f> wet=<f>`
- `:bus fx <bus> none`

Keep parsing explicit and stern in v1. Unknown keys should reject clearly.

**Step 3: Update summaries**

Make `:mixer` and the TUI render compact reverb summaries such as:

```text
bus verb -> master reverb(size=0.75 damp=0.35 wet=1.00)
```

**Step 4: Make session/TUI tests green**

Run:

- `cargo test -p orpheus-lang bus_fx`
- `cargo test -p orpheus-lang --all-targets`

Expected: PASS.

### Task 7: Refresh The Legacy Effect Routing Note

**Files:**
- Modify: `docs/plans/effect_routing.md`

Add a short note at the top explaining that:

- shared delay is implemented on the routing snapshot spine
- shared reverb is the next hosted bus operator
- pattern-language `send(...)` still remains deferred

### Task 8: Full Verification And Sludge Scan

**Step 1: Run full verification**

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-dsp -p orpheus-lang --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-dsp --all-targets`
- `cargo test -p orpheus-lang --all-targets`
- `C:\Users\markm\verus\verus.exe proofs\routing_graph.rs`

**Step 2: Scan touched areas for sludge**

Run:

- `rg -n "TODO|FIXME|Stub:" crates/orpheus-dsp/src/effects crates/orpheus-dsp/src/routing.rs crates/orpheus-dsp/src/engine.rs crates/orpheus-dsp/src/offline.rs crates/orpheus-lang/src/mixer.rs crates/orpheus-lang/src/session.rs crates/orpheus-lang/src/tui.rs docs/plans/effect_routing.md`

**Step 3: Review final shape**

Confirm:

- shared reverb follows the exact hosted-bus snapshot model as delay
- unchanged reverb tails survive unrelated routing changes
- dry routing remains understandable
- TUI summaries stay legible in narrow space
- no extra topology was smuggled into the routing model

## Notes

- Delay and reverb should remain peers in the hosted-bus algebra, not cousins
  with separate semantics.
- Keep the first reverb surface minimal. If the musician wants plate theology
  later, that can arrive after the host model is proven.
- Do not add pre-delay in the same batch. That is a second timing law hiding in
  a nice coat.
