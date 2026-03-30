# 🔭 Vantage: Spec for OSC Protocol Integration

## 👤 User Story
"As a Live Coder, I want to send and receive Open Sound Control (OSC) messages to and from Orpheus, so that I can synchronize visualizers (like TouchDesigner or Processing), control external software synthesizers (like SuperCollider), or accept input from custom hardware controllers, deeply integrating Orpheus into a larger multimedia performance ecosystem."

## ❓ The "So What?" (Business Problem)
Live coding performances are often multimedia experiences. While Orpheus is powerful for audio generation and composition, it currently exists in isolation. If a performer wants to trigger a generative video clip precisely when a kick drum pattern fires, or control the filter cutoff using an iPad interface, they are stuck. MIDI is often too low-resolution and rigidly structured for expressive multimedia mapping. OSC is the industry standard for high-resolution, low-latency network communication between creative coding environments. Adding OSC turns Orpheus from a standalone audio tool into a networked central brain for an entire A/V performance. Complexity is a cost; utility is revenue. Interoperability via OSC acts as a massive utility multiplier, dramatically increasing the appeal of Orpheus to professional digital artists and interdisciplinary performers.

## 🎯 Metric Definition
- **Success** = Orpheus can act as both an OSC server and client. It can transmit outbound pattern events as OSC messages and receive inbound OSC messages to update defined variables, with an average network latency under 5ms on a local loopback, dropping zero packets under typical live coding loads (e.g., 500 messages/sec), all without causing locks, allocations, or audio dropouts on the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Closed ecosystem. All input is via text, and all output is internal audio. There is no network communication capability (aside from planned Ableton Link for clock sync).
- **Competitors (TidalCycles, Sonic Pi, SuperCollider):** All three have OSC deeply embedded in their DNA. TidalCycles uses OSC to communicate with SuperDirt (its audio engine). Sonic Pi explicitly exposes OSC sending and receiving via simple commands (`osc "/play", 60`).
- **The Gap:** Orpheus needs a robust, non-blocking UDP network layer capable of formatting and parsing OSC messages. It needs language-level primitives to target an external IP/Port and a mechanism to ingest incoming OSC paths and bind them to reactive values within the pattern engine.

## ✅ Acceptance Criteria
- Must introduce a command to configure outbound OSC targets (e.g., `:osc target visuals 127.0.0.1:8000`).
- Must support a new `osc("target", "/address/path")` function (or similar syntax) in the language to route a pattern's evaluated events to the network instead of the audio engine.
- Must translate Orpheus event values (Numbers, Notes, Strings) into appropriate OSC data types (Float32, Int32, String) dynamically.
- Must introduce a command to start an internal OSC listener on a specific port (e.g., `:osc listen 9000`).
- Must support binding an incoming OSC path to a reactive variable that can be queried by patterns (e.g., `let cutoff = osc_in("/filter/freq", 440)`).
- Must perform all network I/O and message serialization/deserialization on dedicated background threads, using lock-free message passing to communicate state changes to the audio and evaluation threads.

## 🚫 Out of Scope
- TCP support. Phase 1 will exclusively use UDP, which is the standard for real-time OSC.
- OSC Bundle scheduling based on exact future timestamps. While Orpheus operates on exact rationals internally, Phase 1 will send OSC messages immediately when the audio engine processes the event block, rather than relying on external schedulers to align timestamps.
- Zero-configuration networking (e.g., Bonjour/mDNS discovery). Phase 1 requires explicit IP and Port configuration.
