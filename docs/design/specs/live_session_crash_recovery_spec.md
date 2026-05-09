# 🔭 Vantage: Spec for Live Session Crash Recovery

## 👤 User Story
"As a Live Coder performing on stage, I want Orpheus to continuously auto-save my live session state in the background, so that if the application crashes or my laptop loses power, I can restart and immediately resume my performance with all my current patterns and parameters intact."

## ❓ The "So What?" (Business Problem)
Live coding relies heavily on ephemeral state built up during a performance. A system crash during a live set is a catastrophic failure for an artist. Currently, if Orpheus crashes, the artist loses everything not explicitly saved to an `.ode` file, completely ruining the performance and eroding trust in the software as a professional tool. Complexity is a cost; reliability is a baseline requirement. By implementing robust crash recovery, we reduce the stress on the performer, build immense trust in the platform's stability, and elevate Orpheus from an experimental toy to a stage-ready instrument.

## 🎯 Metric Definition
- **Success** = 100% of the currently playing patterns, evaluated variables, and mixer state are restored within <2 seconds of restarting after an ungraceful shutdown. The auto-save mechanism must add 0 audio dropouts and less than 1ms of overhead to the REPL/UI threads.

## 🔍 Gap Analysis
- **Current State:** Orpheus has zero persistence of active REPL state. All dynamically evaluated variables and active cycles are held in volatile memory. A crash means starting from an empty project.
- **Competitors (Ableton Live, Bitwig):** Standard DAWs have robust crash recovery that restores the project exactly as it was when the crash occurred. TidalCycles relies on the editor's file state, but the underlying Haskell engine state can still be lost on a crash.
- **The Gap:** We need a background mechanism that periodically snapshots the active environment (or streams REPL commands) to a durable disk log without blocking the main event loops.

## ✅ Acceptance Criteria
- Must introduce a background logging thread that appends every successfully evaluated command and its resulting environment diff to a `.orpheus_crash_log` file in a non-blocking manner.
- Must detect the presence of an unsaved crash log on startup and prompt the user (or provide a CLI flag `--recover`) to restore the previous session.
- Must accurately reconstruct the active patterns, variable bindings, and mixer state by replaying the crash log silently during initialization.
- Must cleanly delete the crash log upon a graceful exit of the application.
- Must not introduce any latency or stutter to the audio rendering thread or REPL input responsiveness.

## 🚫 Out of Scope
- Full "Time Travel" undo/redo history (this is about crash recovery, not reversing intentional mistakes).
- Saving or restoring external OS state (e.g., reconnecting external MIDI devices if they were the cause of the crash).
- Recovering audio buffers that were mid-render at the exact moment of the crash.
