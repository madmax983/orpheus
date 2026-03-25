# 🔭 Vantage: Spec for MIDI Input Support

## 👤 User Story
"As a Live Coder, I want to use external MIDI controllers (keyboards, drumpads, knob boxes) to manipulate my Orpheus patterns, parameters, and live session state, so that I can have tactile, expressive control over my performance rather than relying solely on typing code."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus is controlled entirely via keyboard input into the REPL or TUI. While code is unparalleled for structural composition and pattern generation, it lacks the immediacy and tactile feedback of physical knobs, faders, and keys. This is especially true for continuous parameter manipulation, like filter sweeps or live mixing (gain/pan adjustments). By not supporting MIDI input, Orpheus isolates itself from the performer's physical environment and standard electronic performance workflows. Complexity is a cost, but limiting expressive control restricts the artist. Adding MIDI input transforms Orpheus into a hybrid instrument, seamlessly bridging the gap between algorithmic code and human physical expression, making the platform much more appealing to traditional electronic musicians and live performers.

## 🎯 Metric Definition
- **Success** = Orpheus can discover and connect to standard MIDI input devices, map incoming MIDI CC (Control Change) messages to active pattern parameters or mixer settings with <5ms of latency, and capture MIDI Note On/Off events as readable streams in the language, without causing any blocking operations, allocations, or dropouts on the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The system is completely blind to external control surfaces. All parameter changes and triggers must be explicitly typed as code during a live session.
- **Competitors (Sonic Pi, TidalCycles):** Sonic Pi can receive incoming MIDI notes and CC values to trigger synths or alter state (via `sync "/midi/..."`). TidalCycles can receive MIDI via SuperCollider/SuperDirt to modulate pattern parameters in real-time.
- **The Gap:** Orpheus needs a low-latency MIDI input layer that translates asynchronous incoming MIDI messages into reactive values or events that can be seamlessly referenced within the language and mixer architecture.

## ✅ Acceptance Criteria
- Must introduce REPL/TUI commands to list (`:midi in list`) and connect to (`:midi in connect "Port Name"`) available system MIDI input ports.
- Must provide a language primitive to read continuous control values (e.g., `cc(1)` or `midi_cc(1)`) and map them dynamically to pattern parameters (e.g., `|> cutoff(cc(1))`).
- Must provide a mechanism to map incoming MIDI Note On/Off events to trigger specific patterns or functions.
- Must process incoming MIDI messages on a dedicated background thread and update reactive state using lock-free data structures (like `ArcSwap` or atomic variables) to ensure the audio rendering thread is never blocked.
- Must gracefully handle MIDI controller disconnections during a live session without crashing.

## 🚫 Out of Scope
- Bi-directional control surface support (e.g., sending state back to the controller to update motorized faders or LED rings). Phase 1 is strictly unidirectional input to the engine.
- MPE (MIDI Polyphonic Expression) support. Phase 1 focuses exclusively on standard Note On/Off, Pitch Bend, and CC messages.
