# Routing And Mixer Architecture Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build phase-1 routing for Orpheus with explicit tracks, named buses, immutable routing snapshots, cycle-safe render-thread swaps, and read-only mixer inspection without breaking current live-coding flow.

**Architecture:** Add a new `orpheus-dsp` routing domain centered on a validated immutable `RoutingSnapshot`, and have the session/mixer control plane compile those snapshots off the audio thread. The render engine should continue using lock-free command handoff, but swap complete routing snapshots only at cycle boundaries. To preserve current DX while explicit tracks arrive, phase 1 includes a reserved compatibility track named `main` that continues to receive the most recent live sample binding until the user opts into explicit routing.

**Tech Stack:** Rust 2024, `orpheus-dsp`, `orpheus-lang`, `orpheus-pattern`, `rtrb`, cargo fmt/clippy/test, Verus proofs in `proofs/`.

---

### Task 1: Add Red Proofs And Routing Domain Tests

**Files:**
- Create: `proofs/routing_graph.rs`
- Create: `crates/orpheus-dsp/tests/routing_snapshot.rs`

**Step 1: Write the failing proof skeleton**

Create `proofs/routing_graph.rs` with a small arithmetic/control model for phase-1 routing:

- track nodes
- bus nodes
- master sink
- allowed edge kinds
- acyclic termination at master

Target lemmas:

- a valid phase-1 route never traverses `bus -> bus`
- every valid route ends at master
- a track can send to zero or more buses, but buses do not feed tracks
- selector-like grouping is outside the render model

**Step 2: Write the failing routing snapshot tests**

Add tests for:

```rust
#[test]
fn routing_snapshot_accepts_main_track_to_master() {
    let snapshot = RoutingSnapshot::builder()
        .main_track()
        .build()
        .unwrap();
    assert_eq!(snapshot.track_count(), 1);
}

#[test]
fn routing_snapshot_rejects_bus_to_bus_edges() {
    let error = RoutingSnapshot::builder()
        .track("drums")
        .bus("verb")
        .bus("delay")
        .send("drums", "verb", 0.5)
        .route_bus_to_bus("verb", "delay")
        .build()
        .unwrap_err();
    assert!(error.to_string().contains("bus-to-bus"));
}

#[test]
fn routing_snapshot_rejects_duplicate_track_names() {
    let error = RoutingSnapshot::builder()
        .track("drums")
        .track("drums")
        .build()
        .unwrap_err();
    assert!(error.to_string().contains("duplicate"));
}
```

Also cover:

- send to unknown bus rejects clearly
- unbound tracks are allowed but silent
- a bus routed to master is accepted

**Step 3: Run the proof and tests to verify red**

Run:

- `C:\Users\markm\verus\verus.exe proofs\routing_graph.rs`
- `cargo test -p orpheus-dsp --test routing_snapshot`

Expected:

- Verus FAIL because the proof file is still stubbed
- test FAIL because `RoutingSnapshot` and related types do not exist

### Task 2: Implement The Routing Domain Model

**Files:**
- Create: `crates/orpheus-dsp/src/routing.rs`
- Modify: `crates/orpheus-dsp/src/lib.rs`
- Modify: `crates/orpheus-dsp/src/command.rs`

**Step 1: Add the routing types**

Create the initial domain model in `routing.rs`:

```rust
pub struct TrackId(u32);
pub struct BusId(u32);

pub struct TrackState {
    id: TrackId,
    name: Box<str>,
    source: TrackSource,
    level: f32,
    pan: f32,
    muted: bool,
    sends: Box<[SendRoute]>,
}

pub struct BusState {
    id: BusId,
    name: Box<str>,
}

pub struct SendRoute {
    bus_id: BusId,
    level: f32,
}

pub enum TrackSource {
    Unbound,
    SamplePattern(Box<[Event<SampleTrigger>]>),
}

pub struct RoutingSnapshot {
    tracks: Box<[TrackState]>,
    buses: Box<[BusState]>,
    master_track_ids: Box<[TrackId]>,
    master_bus_ids: Box<[BusId]>,
}
```

Keep phase 1 intentionally narrow:

- sample-pattern sources only
- buses are dry routing destinations in this slice
- no insert chains yet
- no bus-to-bus edges

**Step 2: Add validation and builder helpers**

Add:

- `RoutingError`
- `RoutingSnapshot::builder()`
- validation for duplicate names, missing endpoints, invalid levels, and forbidden edge kinds
- `RoutingSnapshot::main_only()` or equivalent helper for the compatibility track

**Step 3: Export the routing surface**

Re-export the new types from `crates/orpheus-dsp/src/lib.rs`.

Add any command-side helper structs in `command.rs` that will later travel through `EngineCommand`, but do not wire the engine yet.

**Step 4: Make the proof and tests green**

Run:

- `C:\Users\markm\verus\verus.exe proofs\routing_graph.rs`
- `cargo test -p orpheus-dsp --test routing_snapshot`

Expected: PASS.

### Task 3: Add Red Engine Tests For Snapshot Swaps And Dry Routing

**Files:**
- Modify: `crates/orpheus-dsp/tests/engine_commands.rs`

**Step 1: Write failing engine tests**

Add deterministic render tests for:

```rust
#[test]
fn engine_preserves_main_track_compatibility_path() {
    // Existing single-pattern flow should still produce audible output.
}

#[test]
fn engine_mixes_two_tracks_routed_to_master() {
    // Two independent tracks should sum at master.
}

#[test]
fn engine_applies_track_send_to_dry_bus() {
    // A routed send should add an extra copy through the bus path.
}

#[test]
fn routing_snapshot_activates_only_at_cycle_boundary() {
    // Pending routing should not take effect mid-cycle.
}
```

Use small, deterministic sample patterns so expected sums can be asserted exactly.

**Step 2: Run the tests to verify red**

Run: `cargo test -p orpheus-dsp --test engine_commands routing_`

Expected: FAIL because the engine still only knows about one active pattern and no routing snapshot.

### Task 4: Implement Engine Routing State And Snapshot Adoption

**Files:**
- Modify: `crates/orpheus-dsp/src/command.rs`
- Modify: `crates/orpheus-dsp/src/engine.rs`
- Modify: `crates/orpheus-dsp/src/scheduler.rs`
- Modify: `crates/orpheus-dsp/src/voice.rs`

**Step 1: Extend the command surface**

Add new engine commands:

```rust
pub enum EngineCommand {
    // existing commands...
    SwapRoutingSnapshot(RoutingSnapshot),
}
```

If needed, add a small helper type for queued mixer/routing swaps. Keep the command payload fully allocated before enqueue so the render thread only swaps ownership.

**Step 2: Replace single-pattern state with routing snapshot state**

Refactor `EngineCore` from:

- `active_pattern`
- `pending_pattern`

to:

- `active_routing`
- `pending_routing`

and keep a compatibility `main` track in the default snapshot.

Also evolve `TransportSnapshot` so the UI can tell whether a routing swap is pending. A minimal addition such as `has_pending_routing` is enough for this slice.

**Step 3: Schedule events from all active tracks**

At cycle start:

- adopt the pending routing snapshot if present
- schedule unit-cycle events for every bound sample track in the active snapshot
- preserve current cycle-boundary semantics

Carry `TrackId` through scheduled triggers so the mixer path can attribute voice output to tracks.

**Step 4: Render through track and bus accumulators**

Replace the single `mix_voices(...)` path with a routing-aware frame render:

- sum active voices into per-track stereo accumulators
- apply track mute/level/pan
- send track signal to master and to any bus sends
- sum bus accumulators into master
- write master to the output frame

Important: do not allocate during render. Either:

- keep per-snapshot boxed scratch buffers and swap them in with the snapshot, or
- store pre-sized scratch buffers in engine state and resize only off-thread before the swap

**Step 5: Run engine tests**

Run: `cargo test -p orpheus-dsp --test engine_commands routing_`

Expected: PASS.

### Task 5: Add Offline Render Parity For Routed Mixes

**Files:**
- Modify: `crates/orpheus-dsp/src/offline.rs`
- Modify: `crates/orpheus-dsp/tests/offline_render.rs`

**Step 1: Write failing offline routing tests**

Add tests for:

- two master-routed tracks sum correctly offline
- a dry bus send produces the same summed result offline as live
- muted or unbound tracks stay silent offline

**Step 2: Refactor shared routing mix logic**

Extract the minimum shared helper needed so live and offline render use the same routing accumulation law instead of drifting into two similar little religions.

This may justify a new internal helper module if the code starts duplicating:

- track accumulation
- send accumulation
- bus-to-master accumulation

**Step 3: Run offline tests**

Run: `cargo test -p orpheus-dsp --test offline_render routing_`

Expected: PASS.

### Task 6: Add Red Session Tests For Track And Bus Commands

**Files:**
- Modify: `crates/orpheus-lang/src/session.rs`

**Step 1: Write failing session tests**

Add tests for:

```rust
#[test]
fn eval_line_keeps_using_main_track_by_default() {
    // Existing `drums = bd sn` flow should remain audible.
}

#[test]
fn track_and_bus_commands_compile_a_routing_snapshot() {
    // :track new drums
    // :track bind drums groove
    // :bus new verb
    // :send drums verb 0.35
}

#[test]
fn mixer_command_reports_track_assignments_and_sends() {
    // :mixer should show human-readable routing state.
}
```

Also cover:

- binding a track to an unknown binding rejects clearly
- sending to an unknown bus rejects clearly
- duplicate track or bus creation rejects clearly
- unbinding or rebinding a track silences the old path instead of leaving haunted output

**Step 2: Run tests to verify red**

Run: `cargo test -p orpheus-lang session::tests::track_`

Expected: FAIL because the session has no mixer control plane yet.

### Task 7: Implement Session-Side Mixer State And Routing Compilation

**Files:**
- Create: `crates/orpheus-lang/src/mixer.rs`
- Modify: `crates/orpheus-lang/src/session.rs`
- Modify: `crates/orpheus-lang/src/lib.rs`

**Step 1: Add a session-side mixer model**

Create `mixer.rs` to own mutable control-plane state:

- track definitions
- bus definitions
- compatibility `main` track
- track-to-binding assignment
- track levels/mute
- send levels

This module should compile a concrete `orpheus_dsp::RoutingSnapshot`.

Keep selector support trivial in phase 1:

- exact track names only
- no wildcards or family selectors yet

**Step 2: Add REPL commands**

Extend `eval_command(...)` with at least:

- `:track new <name>`
- `:track bind <track> <binding>`
- `:track level <track> <value>`
- `:track mute <track> <on|off>`
- `:bus new <name>`
- `:send <track> <bus> <level>`
- `:mixer`

Use a single source of truth in the mixer model. Command handlers should mutate the mixer model, compile a fresh snapshot, and enqueue `EngineCommand::SwapRoutingSnapshot(...)`.

**Step 3: Preserve current live-coding behavior**

When a sample binding is evaluated through `eval_line(...)`, continue updating the compatibility `main` track unless the user has explicitly rebound it. This keeps the current "type code and hear it" path alive while explicit tracks land.

**Step 4: Run session tests**

Run: `cargo test -p orpheus-lang session::tests::track_`

Expected: PASS.

### Task 8: Add Minimal Read-Only Mixer Inspection To The TUI

**Files:**
- Modify: `crates/orpheus-lang/src/session.rs`
- Modify: `crates/orpheus-lang/src/tui.rs`

**Step 1: Add a UI-friendly mixer view**

Expose a compact read-only view from the session, for example:

```rust
pub(crate) struct MixerView {
    pub tracks: Vec<MixerTrackLine>,
    pub buses: Vec<MixerBusLine>,
    pub has_pending_routing: bool,
}
```

Keep it summary-shaped, not a full interactive mixer.

**Step 2: Add failing TUI tests**

Add tests that prove:

- the transport/mixer pane shows at least the active track assignment
- pending routing swaps are visible
- bus sends render in a compact human-readable way

**Step 3: Implement the minimal TUI summary**

Render a small routing summary in the transport-side UI, for example:

- `main -> groove -> master`
- `drums -> groove +send verb@0.35`

Do not build interactive faders or performance controls in this slice.

**Step 4: Run TUI tests**

Run: `cargo test -p orpheus-lang tui::tests::mixer_`

Expected: PASS.

### Task 9: Align Routing Documentation

**Files:**
- Modify: `docs/plans/effect_routing.md`
- Modify: `docs/design/orpheus_design.md`

**Step 1: Update the older effect-routing plan**

Add a short note at the top of `docs/plans/effect_routing.md` that shared effects now depend on the routing/mixer architecture and are a follow-on slice above this core.

**Step 2: Update the main design doc**

Adjust the routing/effects language in `docs/design/orpheus_design.md` so it reflects:

- explicit tracks
- buses
- snapshot-based routing swaps

Keep the edits small and factual.

### Task 10: Final Verification

**Files:**
- Review all touched files

**Step 1: Format**

Run: `cargo fmt --all`

Expected: no diff after rerun.

**Step 2: Lint**

Run:

- `cargo clippy -p orpheus-dsp --all-targets --all-features -- -D warnings`
- `cargo clippy -p orpheus-lang --all-targets --all-features -- -D warnings`

Expected: PASS.

**Step 3: Run tests**

Run:

- `cargo test -p orpheus-dsp --all-targets`
- `cargo test -p orpheus-lang --all-targets`

If the tree is stable enough, finish with:

- `cargo test --all-targets --all-features`

Expected: PASS.

**Step 4: Run proofs**

Run:

- `C:\Users\markm\verus\verus.exe proofs\routing_graph.rs`
- `C:\Users\markm\verus\verus.exe proofs\pattern_time.rs`
- `C:\Users\markm\verus\verus.exe proofs\cycle_query.rs`

Add adjacent routing/timing proofs here if the implementation touches them.

Expected: `0 errors`.

**Step 5: Scan for local sludge**

Run a focused search in the touched area for:

- `TODO`
- `FIXME`
- `Stub:`
- placeholder routing summaries or temporary compatibility hacks that should be documented if retained

Expected: no accidental scaffolding left behind.

---

## Notes For Execution

- Keep the routing core and the first shared-bus/effect host separate. This plan stops at dry routing plus mixer control. Shared reverb/delay processors should be implemented on top of this spine, not tangled into it.
- Treat the compatibility `main` track as a migration shim, not as the long-term ontology. The architecture remains explicit-track-first even if the first implementation preserves current DX.
- Avoid exposing selector/orbit syntax in this slice. Get concrete tracks and buses correct first, then layer selector algebra on top later.
