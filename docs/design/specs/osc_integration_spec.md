# 🔭 Vantage: Spec for OSC (Open Sound Control) Integration

## 👤 User Story
"As a Live Coder, I want to send and receive Open Sound Control (OSC) messages, so that I can synchronize Orpheus patterns with external audio/visual software (like TouchDesigner, SuperCollider, or Resolume) and control visual generators in real-time."

## ❓ The "So What?" (Business Problem)
Live coding performances are often audio-visual. Relying solely on Orpheus's internal audio limits the visual aspect of performances. Without OSC, performers must use complex internal loopbacks or MIDI (which is low-resolution and lacks semantic paths) to drive visuals. OSC provides a high-resolution, network-transparent protocol. Adding OSC integration unlocks the ability for Orpheus to act as the "brain" of a larger A/V installation, instantly appealing to VJs, creative coders, and multimedia artists. Complexity is a cost, but interoperability is an ecosystem multiplier.

## 🎯 Metric Definition
- **Success** = Users can define an OSC target and send patterned messages with < 2ms network scheduling jitter.
- **Success** = Orpheus can trigger patterns or update global variables based on incoming OSC messages.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Isolated. Only communicates with the local audio driver and TUI.
- **Competitors (TidalCycles, Sonic Pi):** Both treat OSC as a first-class citizen (Tidal uses it to talk to SuperDirt; Sonic Pi has `osc` and `sync` functions).
- **The Gap:** Orpheus lacks network-based control protocols, making it a siloed tool rather than a modular component in an A/V pipeline.

## ✅ Acceptance Criteria
- Must introduce a built-in function to define OSC output targets (e.g., `oscTarget("127.0.0.1", 8000, "/address")`).
- Must support sequencing OSC messages from patterns, mapping Orpheus data types (Numbers, Strings) to OSC arguments.
- Must provide an API to listen for incoming OSC messages on a configurable port and map them to global session variables or trigger events.
- Must execute outgoing OSC messages concurrently without blocking the real-time audio DSP thread.

## 🚫 Out of Scope
- Creating a visualizer inside Orpheus (Orpheus remains text/audio focused, external tools handle visuals).
- Custom transport protocols over OSC (Phase 1 relies on UDP only).
- Automatic discovery of OSC nodes via ZeroConf/Bonjour.
