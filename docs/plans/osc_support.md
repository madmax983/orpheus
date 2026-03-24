# 🔭 Vantage: Spec for OSC Support

## 👤 User Story
"As a Live Coder, I want to send and receive Open Sound Control (OSC) messages from Orpheus, so that I can control external software (like SuperCollider, visualizers, or Max/MSP) and receive external control signals (like from a Lemur or TouchOSC interface) using Orpheus's pattern language."

## ❓ The "So What?" (Business Problem)
Orpheus is currently a self-contained audio environment. While this is great for standalone composition, modern live coding performances often involve multi-modal components like reactive visuals (e.g., Hydra, TouchDesigner) or complex synthesis engines (SuperCollider, external DAWs). MIDI output solves notes, but OSC provides high-resolution, high-bandwidth parameter control over a network. Without OSC, users cannot easily sync their Orpheus patterns to generate visuals or use advanced custom interfaces to manipulate their code in real-time. Complexity is a cost, and utility is revenue. Adding OSC support expands Orpheus from an audio sequencer into a multimedia control hub, massively increasing its utility for professional audiovisual artists.

## 🎯 Definition of Success
- **Success** = Orpheus can send formatted OSC messages to a specified network IP/port triggered by pattern events, and can receive incoming OSC messages to modify variables or trigger REPL commands, with latency under 10ms and zero allocations on the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Inter-application communication is limited. There's MIDI output support (planned/implemented), but no high-resolution or networked data exchange mechanism for arbitrary parameters.
- **Competitors (TidalCycles, Sonic Pi):** Both rely heavily on OSC. TidalCycles uses OSC to communicate with SuperDirt (SuperCollider). Sonic Pi has built-in `osc_send` and `sync` features for network communication.
- **The Gap:** Orpheus needs an OSC input/output layer. It must be able to translate cycle-based pattern events into outbound OSC packets, and provide a listener thread to map incoming OSC messages to Orpheus state changes.

## ✅ Acceptance Criteria
- Must introduce a command (e.g., `:osc connect "127.0.0.1:57120"`) to define an OSC target destination.
- Must support a new `osc("address")` destination in the language (or similar syntax) to route a pattern to the OSC output rather than the audio bus.
- Must translate Orpheus control patterns (e.g., `Number`, `String`) into standard OSC arguments (floats, ints, strings).
- Must provide a mechanism to listen for incoming OSC messages on a specified port (e.g., `:osc listen 8000`).
- Must allow mapping incoming OSC messages to Orpheus REPL evaluations or pattern parameter updates.
- Must execute OSC I/O on a dedicated background thread to ensure zero locks, allocations, or blocking on the real-time audio generation thread.

## 🚫 Out of Scope
- A visual node editor for OSC mapping. Phase 1 is code-based routing only.
- Complex OSC query or discovery protocols (like ZeroConf/Bonjour). Phase 1 will use explicit IP and port configurations.
- Automatic bidirectional state synchronization with other Orpheus instances over the network.
