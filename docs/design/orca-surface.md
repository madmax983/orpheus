# Orca-Inspired Grid Surface for Orpheus

**Author:** Mark
**Status:** R&D Spike — exploratory design, not a committed roadmap item
**Date:** July 2026

---

## 0. What this document is

This is the design half of an R&D spike investigating whether an
[Orca](https://100r.co/site/orca.html)-style 2D grid livecoding surface can sit
on top of Orpheus's rational-time pattern model without bending either side.
The spike deliverable is (a) this document and (b) a self-contained, TUI-free
grid engine prototype at `crates/orpheus-lang/src/orca/` that proves the
semantics and the time-mapping seam compile and pass tests against the real
`orpheus-pattern` types. **Audio wiring is deliberately out of scope** (see
section 6).

Orca semantics referenced below were verified against the canonical
JavaScript implementation (`hundredrabbits/Orca`, branch `main`).

---

## 1. Mapping the grid/frame model onto rational pattern time

### 1.1 The two clocks

Orca's world advances in discrete *frames*: one frame is one rewrite of the
whole grid, and in the reference client one frame is one 16th note (4 frames
per beat). Orpheus's world is continuous rational time: a pattern is an
infinite function of time queried over half-open `TimeSpan`s, with one musical
cycle being the interval `[n, n+1)` and one cycle equal to 4 beats
(`BEATS_PER_CYCLE`, `orpheus-dsp/src/engine.rs`).

These reconcile exactly. Pick `F` = grid frames per cycle (Orca's convention
of 16th-note frames over a 4-beat bar gives `F = 16`; the engine keeps `F`
configurable). Then grid frame `N` occupies the rational span:

```text
frame N of F  ->  TimeSpan [ N/F , (N+1)/F )
```

built with `Rational::checked_from_parts(N, F)` and
`TimeSpan::new(start, end)`. No floats appear anywhere; the pattern model was
built for exactly this. The prototype exposes this as
`orca::frame_span(frame, frames_per_cycle) -> Result<TimeSpan, PatternError>`
and tests it against the real `orpheus-pattern` types — this function is the
key seam of the whole design.

### 1.2 The grid as a generator

The grid is a *generator*: a deterministic function from (initial grid state,
frame index) to emitted events. Running one frame produces zero or more
structured events (`OrcaEvent`: frame index, grid position, note glyph, base-36
value). Materializing one cycle means ticking frames `0..F` and stamping each
event with its `frame_span`.

That materialized output is exactly the shape of an
`EventStream<T>`/`Vec<Event<T>>` — a finite, non-repeating list of events over
one unit cycle. This matters because the entire existing publication path is
built on materialized unit-cycle event lists:

```text
grid ticks 0..F
  -> Vec<OrcaEvent>
  -> Vec<Event<SampleEvent>>                    (each event's part = frame_span(N, F))
  -> SamplePatternValue::from_events            (value.rs)
  -> ReplSession::push_pattern_update           (session.rs)
  -> PatternUpdate / EngineCommand::LoadPattern (orpheus-dsp command.rs)
  -> Scheduler::schedule_cycle_events           (rational -> sample frames)
```

Zero engine changes are needed: the engine already re-schedules a unit-cycle
event list at every cycle boundary (`begin_cycle`), and
`rational_to_frame_offset` already performs the exact rational-to-frame
conversion. The grid inherits tempo for free because it speaks cycle time,
not milliseconds.

Because grid evolution is deterministic, the grid could later implement
`Pattern<SampleEvent>` directly (`query(span)` simulates the frames
overlapping `span`), making `fast`, `rev`, exports, etc. work on grids
unchanged. That requires the grid state at frame N to be a pure function of
(initial grid, N) — true for the classic operator set; random operators would
need a frame-keyed seeded PRNG. Deferred (see section 6).

A subtlety for later: replaying one materialized cycle every cycle assumes the
grid is `F`-periodic, which running grids generally are not (a moving `E`
does not return home). v1 must either re-publish the next cycle's
materialization at each cycle boundary (the TUI already polls
`TransportSnapshot` every 50 ms, so publishing one cycle ahead is cheap) or
adopt a per-cycle generator track source in the engine — the latter is an
engine-core change and deserves its own ADR.

### 1.3 Bangs vs. notes in event terms

Only *output* operators emit events off the grid (in real Orca: MIDI/OSC/UDP
operators). Bangs (`*`) are intra-grid control flow and never leave the
engine. An emitted note event gets `part = frame_span(N, F)`; a pure trigger
could equally use a zero-width span at `N/F` (zero-length spans are legal per
ADR 0002), but v0 uses the full frame span since sample triggers carry
duration downstream.

---

## 2. Faithful Orca semantics preserved

The prototype keeps the reference implementation's core rules exactly,
because livecoding idioms (bang trains, collision triggers, halt tricks)
depend on them:

1. **Row-major single-pass evaluation with immediate writes.** One frame scans
   top-left to bottom-right; each operator reads/writes the shared grid
   immediately, so earlier operators see last frame's values and later
   operators see values already written this frame.
2. **Per-frame lock set.** Locks reset at frame start. A locked cell's
   operator is skipped. Operators lock their operand and output cells, which
   is (a) how operands act as data even when they are letters, (b) why a
   freshly written output never executes in the frame it was produced, and
   (c) why a moving operator is not re-executed when the scan reaches its
   destination.
3. **Case selects execution mode, not value.** Uppercase operators run every
   frame; lowercase operators run only when a `*` sits in a cardinal (E/W/S/N,
   no diagonals) neighbor cell at the moment the scan reaches them.
4. **`*` self-erases.** A bang is itself an operator whose only action is to
   delete itself: it lives on the grid for exactly one frame.
5. **Base-36 values.** `.`/`*` read as 0; `0`-`9` as 0-9; `a`/`A`-`z`/`Z` as
   10-35 (case does not affect value); all arithmetic wraps mod 36.
6. **Case-sensitive outputs.** Arithmetic outputs are uppercased iff the glyph
   in the operator's right-hand cell is an uppercase letter — the mechanism
   for computing a *live* (uppercase) vs. *dormant* (lowercase) operator.
7. **Movement explodes.** `N S E W` move one cell per frame (or per bang when
   lowercase); moving into any non-empty cell or out of bounds replaces the
   mover with `*` at its current position. No wrapping (matching orca-js; the
   C port's wrap mode is not assumed).

---

## 3. v0 operator set and exact semantics

Eight operators plus the bang, chosen to exercise every semantic mechanism
(movement/collision, arithmetic/case rule, frame-clocked state, bang
production, bang consumption, event output):

| Glyph | Name | Inputs | Output | Semantics |
|---|---|---|---|---|
| `N` `S` `E` `W` | movement | — | — | Move 1 cell north/south/east/west into an empty (`.`) destination; erase origin, write self at destination, lock destination. Non-empty destination or out-of-bounds: replace self with `*` (explode). Lowercase variants move only on bang. |
| `A` | add | left `{-1,0}`, right `{+1,0}` | below `{0,+1}` | `keyOf((a + b) mod 36)` written below every frame; output uppercased iff right glyph is an uppercase letter. Operand and output cells locked. |
| `C` | clock | rate `{-1,0}` (min 1), mod `{+1,0}` | below `{0,+1}` | `keyOf(floor(frame / rate) mod m)` written below every frame; case rule as `A`. Empty mod writes nothing (matching main-branch orca-js, which NaNs out — a deliberate divergence from Orca-c's default-8). |
| `D` | delay | rate `{-1,0}` (min 1), mod `{+1,0}` (min 1) | below `{0,+1}`, **bang** | Writes `*` below when `frame mod (rate*m) == 0` or `m == 1`, else writes `.`; output cell locked either way. Phase-locked to the global frame counter. |
| `*` | bang | — | — | Self-erases when the scan reaches it; triggers lowercase operators and output operators in its cardinal neighborhood for exactly one frame. |
| `:` | out (simplified) | note `{+1,0}` | event | Locks its note cell every frame; when a `*` is cardinal-adjacent, pushes `OrcaEvent { frame, x, y, note, value }` into the engine's per-tick event vector. This is a deliberately reduced form of Orca's `:` MIDI operator (channel/octave/velocity/length ports deferred). |
| `0`-`9` | data | — | — | Inert; never execute. |

Everything else on the grid (`.` empty, unrecognized letters) parses to a
no-op in v0.

Known scan-order asymmetry, preserved deliberately: a *hand-placed* orphan `*`
only triggers operators evaluated before it in scan order (operators for which
the bang lies east or south), because it self-erases when the scan reaches it.
Producer-written bangs (`D`) are locked by the producer and propagate reliably
in all four directions. This matches orca-js exactly and is covered by tests.

---

## 4. Module location: `crates/orpheus-lang/src/orca/`

Decision: a module inside `orpheus-lang`, not a new crate — with the grid
engine kept engine-pure (no TUI imports, depending only on `orpheus-pattern`
and std) so it can be extracted later. Rationale (from the architecture
study):

1. **Dependency direction already works.** The engine needs `orpheus-pattern`
   types, and its output ultimately targets `orpheus-dsp` types — both already
   dependencies of `orpheus-lang`. A separate crate buys isolation but not
   decoupling: TUI hosting and session integration must live in
   `orpheus-lang` regardless.
2. **Session integration is crate-private.** `push_pattern_update`,
   `MixerState`, undo history are `pub(crate)` in `orpheus-lang`; a module
   calls them directly, a crate would force premature public API.
3. **Precedent.** Sizable sub-surfaces already live as lang modules (`tui/`,
   `types/`, `tracker.rs`); crates are reserved for layers with a one-way
   dependency story. Orca sits *on top of* the stack, not beside it.
4. **Escape hatch.** `orca/engine.rs` + `orca/grid.rs` have no TUI or session
   imports. If the operator set grows to deserve its own proofs/crate, they
   lift out into `crates/orpheus-orca` depending only on `orpheus-pattern`.

Spike layout (implemented):

```text
crates/orpheus-lang/src/orca/
  mod.rs        # module docs + re-exports
  grid.rs       # Grid: base-36 glyph cells, bounds, get/set
  engine.rs     # OrcaEngine: frame tick, locks, operators, OrcaEvent, frame_span
```

v1 would add `publish.rs` (OrcaEvent -> Event<SampleEvent> -> session) and
`pane.rs` (TUI grid editor).

---

## 5. TUI hosting sketch (not implemented)

The architecture study identifies the extension point: TUI panes are plugins
implementing `ratatui_hypertile_extras::HypertilePlugin`, registered by name
via `runtime.register_plugin_type` in `tui/mod.rs` (`build_runtime`), each
factory capturing the shared `Rc<RefCell<SharedState>>` that holds the
`ReplSession`. An Orca grid editor is therefore one new plugin
(`orca/pane.rs`) plus one registration call — spawnable from the existing
command palette, no event-loop changes.

Key points for the eventual pane:

- **Cursor/playhead from the engine clock, not a UI timer.**
  `TransportSnapshot::{current_frame, current_cycle_start_frame,
  frames_per_cycle}` gives the current grid frame as
  `((current_frame - current_cycle_start_frame) * F) / frames_per_cycle`,
  sample-accurate at the existing 50 ms poll.
- **Editing** writes glyphs through `Grid::set`; every edit re-materializes
  the next cycle and republishes (section 1.2).
- **Testing** follows the existing `TestBackend` buffer-assert convention
  (`render_initial_frame_for_test`).

---

## 6. Out of scope for this spike

Explicitly not part of v0 (each is a candidate v1+ work item):

- **Audio wiring.** No `SampleEvent`/`SampleTrigger` conversion, no
  `push_pattern_update`, no `EngineCommand`. Events land in a `Vec<OrcaEvent>`
  and stop there. The `frame_span` function proves the seam type-checks.
- **TUI grid editor pane** (section 5 sketches it; nothing rendered).
- **The full A-Z operator set.** `B F G H I J K L M O P Q R T U V X Y Z`,
  comments (`#`), variables, and the halt/lock idioms beyond what the lock
  set already provides.
- **Real IO operators**: MIDI (`:` full port layout, `%`, `!`, `?`), UDP
  (`;`), OSC (`=`), self-command (`$`).
- **`Pattern<T>` implementation for the grid** (queryable grid; needs the
  determinism/PRNG story and a re-publish or generator-track ADR).
- **Multi-cycle / re-publish scheduling**, BPM-linked frame advance,
  transport integration.
- **Language surface**: no `.ode` syntax, no grid literals, no bindings.
- **Persistence** (`.orca` file load/save).
- **Proofs.** Operator invariants (e.g. lock-set soundness: no cell executes
  twice per frame) are natural Verus candidates once the engine shape
  stabilizes; premature for a spike.

## 7. What the spike must prove

1. Orca frame semantics implement cleanly in safe Rust with a per-frame lock
   set and immediate writes (tests: movement, collision, bang lifetime,
   lowercase-on-bang, clock/delay phase, base-36 case rule, lock skipping).
2. `frame_span` compiles and behaves against the real `orpheus-pattern`
   `Rational`/`TimeSpan` types — frames land on exact rationals with no
   floats.
3. Grid output materializes into a plain `Vec` of structured events, i.e. the
   shape the existing unit-cycle publication path already consumes.
