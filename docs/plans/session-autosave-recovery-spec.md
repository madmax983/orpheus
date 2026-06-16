# 🔭 Vantage: Spec for Session Autosave and Crash Recovery

## 👤 User Story
"As a Live Coder, I want my active session state to automatically save in the background, so that if my laptop dies or the application crashes mid-performance, I can instantly recover my exact arrangement, bindings, and mixer state upon restarting without losing my creative momentum."

## ❓ The "So What?" (Business Problem)
Live performance environments are inherently high-stakes. A software crash during a live set is a catastrophe. Currently, Orpheus holds all active state (pattern bindings, track routing, bus effects) strictly in volatile memory unless manually exported. If the application panics or power is lost, the entire composition vanishes. This lack of fault-tolerance makes Orpheus a liability for professional, stage-critical use. Complexity is a cost; utility is revenue. Providing a silent, invisible safety net transforms Orpheus from a fragile experiment into a battle-tested, professional-grade instrument that performers can trust.

## 🎯 Metric Definition
- **Success** = Following a forced termination (`SIGKILL`) of the Orpheus process, relaunching the application detects the orphaned autosave state and fully restores the previous bindings and mixer graph in <500ms, resuming audio output exactly where it left off, with the autosave routine running in the background without causing >1ms jitter on the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Session state is ephemeral. While REPL history might be saved as raw text, the actual DSP routing and evaluated bindings are lost upon exit.
- **Competitors (Ableton Live, Bitwig, FL Studio):** All major DAWs feature robust crash recovery, prompting the user to "Recover Session" upon a bad exit.
- **The Gap:** Orpheus lacks an asynchronous mechanism to serialize the current `ReplSession` state to a temporary disk location safely, and lacks a startup routine to detect and offer restoration of that state.

## ✅ Acceptance Criteria
- Must implement a background worker thread that receives lightweight state snapshots from the main session and serializes them to a `.orpheus_autosave` file.
- Must not allocate memory or hold locks on the real-time audio thread during the snapshot or serialization process.
- Must detect the presence of an orphaned `.orpheus_autosave` file on application startup.
- Must prompt the user in the REPL/TUI (e.g., `Unclean shutdown detected. Recover session? [y/N]`) and successfully rebuild the entire state if accepted.
- Must safely delete the `.orpheus_autosave` file upon a clean application exit.

## 🚫 Out of Scope
- Full "Time Machine" version history of autosaves (Phase 1 only keeps the single most recent snapshot).
- Recovering external state like disconnected MIDI hardware or deleted sample files on disk.