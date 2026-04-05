# 🔭 Vantage: Spec for Undo/Redo (Time Travel)

## 👤 User Story
"As a Live Coder, I want the ability to undo and redo my recent code evaluations or arrangement changes, so that I can quickly recover from syntax errors, accidental deletions, or musical ideas that didn't work out without losing my performance flow."

## ❓ The "So What?" (Business Problem)
Live coding is inherently risky; it's performed in real-time, often in front of an audience. An errant keystroke or a badly formed pattern can instantly derail a performance. Currently, if a user accidentally overwrites a complex drum pattern with a typo, that pattern is gone unless they have it saved in an external file or can recreate it from memory. This fear of making mistakes stifles experimentation. A robust undo/redo system (time travel) acts as a safety net. Complexity is a cost, but fear is a paralyzer. Adding undo/redo functionality empowers users to take creative risks, drastically increasing the utility of the environment as an improvisational instrument.

## 🎯 Metric Definition
- **Success** = Users can instantly revert the state of the active session (including pattern bindings, mixer routing, and global settings) to the previous state using a simple command (e.g., `:undo` or a keybinding) with zero audio dropouts or locking on the real-time audio thread, maintaining a history of at least the last 50 state changes.

## 🔍 Gap Analysis
- **Current State (Orpheus):** State changes (evaluating a new pattern, changing a bus routing) are destructive. The previous state is immediately discarded.
- **Competitors (TidalCycles, Sonic Pi, DAWs):** Traditional DAWs (Ableton, Logic) have robust undo histories. Text-based live coding environments often rely on the text editor's native undo (e.g., in VSCode or Emacs), but this only undoes the *text*, not necessarily the evaluated *state* if the user has already sent the code to the engine.
- **The Gap:** Orpheus needs a dedicated state history manager within its engine that tracks the sequence of evaluated environments, allowing the user to seamlessly roll back the actual running audio state to a previous snapshot.

## ✅ Acceptance Criteria
- Must introduce a session history stack that automatically pushes a lightweight snapshot of the current session state (bindings, mixer configuration) immediately before any new evaluation or command that modifies the state.
- Must provide a command in the REPL/TUI (e.g., `:undo` and `:redo`) and corresponding global keybindings to traverse the history stack.
- Must apply the restored state seamlessly on the next cycle boundary to prevent jarring, mid-beat changes.
- Must ensure that capturing and restoring state snapshots does not allocate on or block the real-time audio thread.
- Must limit the history stack size (e.g., 50 or 100 entries) to prevent unbounded memory growth over long sessions.

## 🚫 Out of Scope
- Undoing the actual audio output (e.g., reversing the audio buffer). This is strictly about the compositional *state* (the code/bindings that produce the audio).
- Branching or non-linear undo histories (like a Git tree). Phase 1 is a simple linear stack.
- Persisting the undo history across application restarts. The history is ephemeral for the duration of the current live session.
