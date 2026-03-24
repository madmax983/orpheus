# Shared Delay Bus Implementation Plan

> **For Claude:** Execute this as a SPEC-PROOF-RED-GREEN-REFACTOR slice. Keep
> the bus host wet-return only, preserve unchanged effect state across snapshot
> swaps, and do not widen the routing topology beyond the existing phase-1
> edges.

**Goal:** Add the first real shared effect host to Orpheus by attaching a
tempo-locked delay processor to named buses, with cycle-safe snapshot adoption,
deterministic tests, and REPL/TUI inspection.

**Architecture:** Extend the validated routing snapshot with an optional
per-bus effect descriptor, keep mutable delay state in the render engine,
preserve unchanged bus effect runtime state across unrelated routing swaps, and
surface bus effect control through `:bus fx ...` in the session mixer layer.

**Tech Stack:** Rust 2024, `orpheus-dsp`, `orpheus-lang`, `orpheus-pattern`,
`cargo fmt`, `cargo clippy`, `cargo test`, Verus in `proofs/routing_graph.rs`.

---

### Task 1: Add Red Proof And Routing Tests For Bus Effect Specs

**Files:**
- Modify: `proofs/routing_graph.rs`
- Modify: `crates/orpheus-dsp/tests/routing_snapshot.rs`

**Step 1: Extend the proof surface**

Add a small proof extension that treats a hosted bus effect as a bus-local
annotation, not as a new routing edge kind.

Target lemmas:

- hosted bus effects do not create new allowed topology edges
- valid phase-1 routes still terminate at master
- bus effects do not introduce `bus -> bus` or `bus -> track` paths

**Step 2: Add red routing snapshot tests**

Add tests for:

```rust
#[test]
fn routing_snapshot_accepts_delay_effect_on_named_bus() {
    let snapshot = RoutingSnapshot::builder()
        .track("drums")
        .bus("dub")
        .bus_effect_delay("dub", rational(3, 16), 0.45, 1.0)
        .send("drums", "dub", 0.35)
        .build()
        .unwrap();
    assert!(snapshot.bus("dub").unwrap().effect().is_some());
}

#[test]
fn routing_snapshot_rejects_invalid_delay_feedback() {
    let error = RoutingSnapshot::builder()
        .bus("dub")
        .bus_effect_delay("dub", rational(1, 8), 1.5, 1.0)
        .build()
        .unwrap_err();
    assert!(error.to_string().contains("feedback"));
}

#[test]
fn routing_snapshot_rejects_delay_for_unknown_bus() {
    let error = RoutingSnapshot::builder()
        .bus_effect_delay("dub", rational(1, 8), 0.45, 1.0)
        .build()
        .unwrap_err();
    assert!(error.to_string().contains("unknown bus"));
}
```

Also cover:

- invalid `wet`
- non-positive delay time
- replacing or duplicating hosted effects on one bus

**Step 3: Verify red**

Run:

- `C:\Users\markm\verus\verus.exe proofs\routing_graph.rs`
- `cargo test -p orpheus-dsp --test routing_snapshot`

Expected: FAIL.

### Task 2: Implement Routing Snapshot Bus Effect Specs

**Files:**
- Modify: `crates/orpheus-dsp/src/routing.rs`
- Modify: `crates/orpheus-dsp/src/lib.rs`

**Step 1: Add bus effect descriptor types**

Add:

- `BusEffectSpec`
- `DelaySpec`
- `RoutingError` cases for bad delay effect config

Expose read-only bus effect inspection through `BusView`.

**Step 2: Extend the builder**

Add builder helpers such as:

- `bus_effect_delay(bus_name, time, feedback, wet)`
- `clear_bus_effect(bus_name)` if useful for later integration

Keep one hosted effect per bus in this slice.

**Step 3: Validate and make tests green**

Run:

- `C:\Users\markm\verus\verus.exe proofs\routing_graph.rs`
- `cargo test -p orpheus-dsp --test routing_snapshot`

Expected: PASS.

### Task 3: Add Red Engine Tests For Shared Delay Behavior

**Files:**
- Modify: `crates/orpheus-dsp/tests/engine_commands.rs`
- Modify: `crates/orpheus-dsp/src/offline.rs` tests or add `crates/orpheus-dsp/tests/offline_render.rs`

**Step 1: Add deterministic engine tests**

Add tests for:

```rust
#[test]
fn engine_renders_shared_delay_bus_repeats() {
    // One pulse into a delay bus should produce exact delayed repeats.
}

#[test]
fn bus_effect_changes_adopt_only_at_cycle_boundary() {
    // A pending delay config should not change the current cycle mid-phrase.
}

#[test]
fn unchanged_bus_effect_state_survives_unrelated_send_update() {
    // Recompiling routing without changing dub delay config must preserve tail state.
}
```

Also add an offline/live parity test for the delay host.

Use small impulse-like samples so expected amplitudes can be asserted exactly.

**Step 2: Verify red**

Run:

- `cargo test -p orpheus-dsp --test engine_commands delay_`

Expected: FAIL.

### Task 4: Implement Delay Runtime State And Bus Processing

**Files:**
- Create: `crates/orpheus-dsp/src/effects/mod.rs`
- Create: `crates/orpheus-dsp/src/effects/delay.rs`
- Modify: `crates/orpheus-dsp/src/engine.rs`
- Modify: `crates/orpheus-dsp/src/offline.rs`
- Modify: `crates/orpheus-dsp/src/lib.rs`

**Step 1: Add a simple stereo delay state**

Implement a deterministic delay line with:

- tempo-locked sample offset derived from current cycle length and rational time
- feedback accumulation
- wet scalar on output
- no allocation on render

**Step 2: Attach effect runtime state to buses**

Render-side state should be keyed by stable bus identity from the control plane.
For this slice, preserve runtime state across adopted snapshots when:

- bus name still exists
- effect kind is still `delay`
- delay config is unchanged

Reset the effect state when the bus host changes or disappears.

**Step 3: Process bus effects in the render loop**

After dry track routing:

- accumulate bus input
- process bus effect if present
- mix wet return to master

Keep the host wet-return only.

**Step 4: Make engine tests green**

Run:

- `cargo test -p orpheus-dsp --test engine_commands delay_`
- `cargo test -p orpheus-dsp --all-targets`

Expected: PASS.

### Task 5: Add Red Session And TUI Tests For `:bus fx`

**Files:**
- Modify: `crates/orpheus-lang/src/session.rs`
- Modify: `crates/orpheus-lang/src/tui.rs`

**Step 1: Add session tests**

Add tests for:

```rust
#[test]
fn bus_fx_command_attaches_shared_delay_to_bus() {
    // :bus new dub
    // :bus fx dub delay time=3/16 feedback=0.45 wet=1.0
}

#[test]
fn bus_fx_command_clears_hosted_effect_with_none() {
    // :bus fx dub none
}
```

Also cover:

- unknown bus
- bad rational time syntax
- invalid `feedback` / `wet`

**Step 2: Add TUI tests**

Add tests that prove:

- the transport pane shows hosted bus effect summaries
- pending routing is visible after `:bus fx` changes
- command completion still works for the new surface

**Step 3: Verify red**

Run:

- `cargo test -p orpheus-lang bus_fx`
- `cargo test -p orpheus-lang transport_pane_shows_`

Expected: FAIL.

### Task 6: Implement Session Bus Effect Control And Summary

**Files:**
- Modify: `crates/orpheus-lang/src/mixer.rs`
- Modify: `crates/orpheus-lang/src/session.rs`
- Modify: `crates/orpheus-lang/src/tui.rs`

**Step 1: Extend mixer bus state**

Replace the current bus-name set with a real bus model that can hold:

- bus name
- optional hosted effect config

**Step 2: Add `:bus fx` parsing**

Support:

- `:bus fx <bus> delay time=<num>/<den> feedback=<f> wet=<f>`
- `:bus fx <bus> none`

Parsing can stay simple and explicit in v1. Reject unknown keys and malformed
values clearly.

**Step 3: Compile effect descriptors into routing snapshots**

Update `MixerState::compile_snapshot(...)` so hosted bus effects are emitted into
the DSP routing snapshot.

**Step 4: Update summaries**

Make `:mixer` and `MixerView` render hosted effect summaries compactly.

**Step 5: Make session/TUI tests green**

Run:

- `cargo test -p orpheus-lang bus_fx`
- `cargo test -p orpheus-lang --all-targets`

Expected: PASS.

### Task 7: Update Legacy Effect Routing Note

**Files:**
- Modify: `docs/plans/effect_routing.md`

Add a short note at the top explaining:

- the routing/mixer architecture is now the foundational layer
- the first follow-on implementation slice is the shared delay bus host
- pattern-language `send(...)` remains deferred

### Task 8: Full Verification And Hardening

**Step 1: Run full verification**

Run:

- `cargo fmt --all`
- `cargo clippy -p orpheus-dsp -p orpheus-lang --all-targets --all-features -- -D warnings`
- `cargo test -p orpheus-dsp --all-targets`
- `cargo test -p orpheus-lang --all-targets`
- `C:\Users\markm\verus\verus.exe proofs\routing_graph.rs`

**Step 2: Scan touched areas for sludge**

Run:

- `rg -n "TODO|FIXME|Stub:" crates/orpheus-dsp/src/effects crates/orpheus-dsp/src/routing.rs crates/orpheus-dsp/src/engine.rs crates/orpheus-lang/src/mixer.rs crates/orpheus-lang/src/session.rs crates/orpheus-lang/src/tui.rs docs/plans/effect_routing.md`

**Step 3: Review final shape**

Confirm:

- unchanged delay tails survive unrelated routing changes
- `:bus fx` does not break dry routing
- bus summaries are legible in narrow TUI space
- no extra topology was smuggled into the routing model

## Notes

- Keep the first shared host wet-return only. Do not add a second dry law to
  explain on buses.
- Preserve unchanged bus effect state across snapshot swaps. Otherwise the first
  shared host will feel cursed the minute someone edits a send.
- Do not add reverb in the same batch. Delay is the architectural proof point,
  not the end of the cathedral.
