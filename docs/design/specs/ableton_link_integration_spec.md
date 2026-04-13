# 🔭 Vantage: Spec for Ableton Link Integration

## 👤 User Story
"As a Live Coder, I want to synchronize my Orpheus session with Ableton Link over a local network, so that I can seamlessly jam with other musicians using DAWs, hardware drum machines, or DJ software without manual beatmatching."

## ❓ The "So What?" (Business Problem)
Live coding rarely happens in a vacuum. Musicians often perform in ensembles or hybrid hardware/software setups. Currently, Orpheus relies entirely on its internal clock. If a performer wants to play alongside an Ableton Live user or a modular synth synced via Link, they have to manually tap tempo and nudge phases, which is error-prone and stressful during a live set. Ableton Link is the industry standard for timing synchronization. By omitting it, we restrict Orpheus to solo performances. Implementing Link integration breaks Orpheus out of its silo, exponentially increasing its utility in collaborative environments. Complexity is a cost; utility is revenue. This feature is pure utility.

## 🎯 Metric Definition
- **Success** = Users can toggle Link sync via a simple REPL command (e.g., `:link on`). The Orpheus engine's BPM and cycle phase perfectly match the local Link session. Phase jitter must remain < 2ms, and tempo changes from external peers must smoothly update the internal scheduler without audio dropouts or structural desyncs.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Timing is driven strictly by an internal tick scheduler. There is no awareness of external clocks.
- **Competitors (TidalCycles, Sonic Pi, SuperCollider):** All major live coding environments support Ableton Link natively. TidalCycles relies on SuperDirt's Link integration; Sonic Pi has it built into its core timing model.
- **The Gap:** Orpheus needs a bridge between its internal timing model and Ableton Link's continuous timeline, adjusting the global tempo dynamically based on the network session.

## ✅ Acceptance Criteria
- Must introduce an optional Ableton Link client feature to the application.
- Must add a new user command `:link [on|off]` to enable or disable synchronization.
- When enabled, the timing engine must listen for tempo and phase changes from the Link session and adjust the audio playback cycle duration.
- Must handle tempo ramping gracefully; sudden BPM changes from peers must not cause audio artifacts or panic the timing calculations.
- The user interface must display a visible "LINK" badge in the header/status bar, along with the active synchronized BPM and the number of connected peers.

## 🚫 Out of Scope
- MIDI Clock Sync or SMPTE Timecode (Link solves the network sync problem better; legacy MIDI clock is Phase 2 if demanded).
- Exposing individual pattern cycles to Link (Link only syncs the global tempo and phase/beat grid, not specific track offsets).
