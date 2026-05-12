# ADR 0006: Session Undo/Redo History

## Status

Accepted

## Context

Live evaluation currently mutates the running session destructively. A bad pattern
or mixer command can replace the compositional state before the performer has a
cheap way back.

## Decision

`ReplSession` owns a bounded linear history of session snapshots. Successful
binding evaluations and state-changing commands record the previous snapshot in
an undo stack and clear redo. `:undo` and `:redo` restore snapshots by replacing
the REPL bindings, inferred types, mixer state, sample bank reference, and global
tuning/tempo settings, then enqueue a routing snapshot for the audio engine.

Restored routing is still applied through `EngineCommand::SwapRoutingSnapshot`,
so pattern and mixer restoration reaches the audio thread at the next cycle
boundary. The audio thread receives immutable commands only; it does not capture
history or allocate snapshot state.

The history depth is capped at 50 snapshots for phase 1. Undo history is linear:
creating a new state after undo discards redo.

## Consequences

- REPL users can call `:undo` and `:redo`.
- TUI users can use global `Ctrl-Z` and `Ctrl-Y`.
- History remains ephemeral and process-local.
- MIDI connection state is not part of the phase-1 snapshot because active
  device handles are not cloneable compositional state.
