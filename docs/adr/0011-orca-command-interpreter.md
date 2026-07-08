# ADR 0011: Grid-Driven Global Tempo via the `$` Command Interpreter

- Status: Accepted
- Date: 2026-07-08

## Context

Orca surface v3 deliberately emitted the `$` self operator's message as an
uninterpreted `OrcaIoEvent::Command` string, and v6's transport dispatcher
dropped those events: in the reference implementation the string goes to
`commander.js`, the *UI's* command bar — a host concern, not grid
semantics. v7 adds the host half, which forces one boundary decision:

The flagship command is `$bpm:N`. In the reference client there is exactly
one clock, so a grid that fires `$bpm` retunes *everything* — that
self-conducting quality is the reason the command exists. In Orpheus the
grid is one source among many hanging off a shared engine transport whose
tempo is otherwise set by the user-facing `:tempo` command. Letting a
grid rewrite the global tempo crosses a layering boundary no other
surface crosses: pattern bindings, the tracker, and MIDI input all
*consume* the transport; none of them *drive* it.

Secondary questions bundled with the same decision:

- Which safety clamp applies? `ReplSession::set_tempo` accepts any finite
  positive BPM; a stray glyph could produce `$bpm:1`.
- Do `$play`/`$stop` control the global transport (as in the reference,
  where the clock is the program) or the grid clock?
- Where does interpretation run, given commands are materialized one
  cycle ahead of playback (ADR 0009) and dispatched on the IO thread
  (ADR 0010)?

## Decision

1. **`$bpm`/`$apm` set the global transport tempo.** Reference fidelity
   wins: a grid that cannot conduct is not Orca. The command routes
   through the existing session seam — literally
   `ReplSession::eval_line(":tempo N")` — so it is indistinguishable from
   a typed `:tempo` command downstream: it participates in **undo
   history** (undo reverts a grid-driven tempo change), emits the same
   status message, and hits the same engine `SetTempo` path.
2. **Values clamp to the reference clock's 60-300 BPM range**
   (`clock.js` `setSpeed`). This is simultaneously the faithful behavior
   and the safety clamp: `$bpm:1` plays at 60, never 1. `bpm:0` no-ops
   (falsy in `setSpeed`), and non-numeric values no-op (`parseInt` NaN),
   both matching the reference.
3. **`$play`/`$stop` control the grid clock, not the global transport.**
   Here the multi-source architecture wins over fidelity: stopping the
   engine transport would silence every unrelated pattern. The grid
   commands map onto the same start/stop the `:orca`-hosted TUI toggle
   uses (including MIDI clock start/stop when clock out is enabled).
4. **Interpretation runs on the TUI/session side, at fire time.** The
   dispatcher (IO thread) collects command strings when their wall-clock
   deadlines arrive — so a `$bpm` on grid frame 9 applies when frame 9
   *plays*, not when it was materialized a cycle earlier — and the TUI
   tick drains them into the interpreter. The audio thread is untouched;
   the IO thread never calls into the session.
5. **One parser for all inputs.** Inbound UDP datagrams
   (`UdpCommandListener`, reference input port 49160) feed the same
   `parse_command`/`apply_orca_command` path, exactly as the reference
   pipes UDP input into `commander.trigger`. Commands with no Orpheus
   mapping and unknown commands no-op with a status-line note.

## Consequences

- A running grid can retune the whole session; the clamp bounds the blast
  radius and undo makes it reversible. This is a *deliberate* new
  capability, documented in the design doc's command table
  (`docs/design/orca-surface.md` section 13.1).
- A `$bpm` banged every frame floods undo history with tempo snapshots
  (16 per cycle at defaults). Accepted: it matches what typing `:tempo`
  sixteen times would do, and idiomatic grids fire commands from `D`/`F`
  gates, not free-running bangs.
- Because commands apply at fire time but cycles materialize one cycle
  ahead, a tempo change affects the *scheduling* of subsequent cycles
  only; events of the already-scheduled cycle keep their deadlines (up to
  one grid cycle of stale spacing, self-correcting at the next boundary).
  The MIDI clock period is refreshed both immediately on `$bpm` and on
  every scheduled cycle, so clock out re-converges within a cycle too.
- `frame`/`rewind`/`skip` mutate only the grid engine's frame counter
  (clamped to the reference's 0-9999999) and need no session involvement.
