# 🔭 Vantage: Spec for Ableton Link Synchronization

## 👤 User Story
"As a Live Coder, I want to synchronize Orpheus's tempo and phase with other software and hardware on my local network using Ableton Link, so that I can seamlessly jam and perform collaboratively with other musicians running Ableton Live, Traktor, or mobile music apps without manual beatmatching or complex MIDI clock routing."

## ❓ The "So What?" (Business Problem)
Music is inherently collaborative, but live coding often isolates the performer within their own terminal and internal clock. If Orpheus cannot sync with external software, it is confined to being a standalone instrument. Manually matching tempo is difficult and phase sync is nearly impossible to maintain over long performances. By integrating Ableton Link, a widely adopted industry standard for timing over local networks, we instantly unlock multiplayer capabilities. Complexity is a cost; utility is revenue. Adding Link support transforms Orpheus from a solitary composition tool into a network-ready performance instrument, drastically increasing its appeal to artists performing in bands or hybrid electronic setups.

## 🎯 Metric Definition
- **Success** = Orpheus maintains tempo and beat phase synchronization with an external Ableton Link session over Wi-Fi/Ethernet with <3ms of jitter relative to the network clock, and responds to external tempo changes within 1 cycle, with zero audio dropouts or blocking operations on the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The internal DSP engine and scheduler rely strictly on a closed, internal exact-rational cycle timing and sample clock. There is no concept of external tempo control.
- **Competitors (TidalCycles, Strudel, Sonic Pi):** TidalCycles integrates with Link via SuperDirt/Carabiner. Sonic Pi has built-in Ableton Link support.
- **The Gap:** Orpheus needs a low-overhead network synchronization layer that translates the continuous Ableton Link timeline into its exact-rational cycle boundaries, allowing its internal scheduler to speed up, slow down, and align its "downbeat" with the network session without disrupting audio playback.

## ✅ Acceptance Criteria
- Must introduce a command in the REPL/TUI (e.g., `:link enable` and `:link disable`) to join or leave an Ableton Link session on the local network.
- Must accurately follow tempo changes initiated by other peers on the network.
- Must accurately broadcast tempo changes initiated within Orpheus (e.g., `set_tempo(120)`) to the rest of the network peers.
- Must align the start of Orpheus's cycle (`Rational::zero()`) with the musical downbeat of the Link timeline.
- Must clearly indicate the current Link status (connected peers, synced tempo) within the TUI transport pane.
- Must perform network communication and clock adjustment on a background thread, using lock-free data structures (like `SharedTransport`) to communicate tempo and phase corrections to the audio render thread.

## 🚫 Out of Scope
- MIDI Clock Sync / MTC over USB/DIN. Phase 1 is strictly IP-based Ableton Link.
- Remote code evaluation or shared REPL sessions across the network. Phase 1 synchronizes time, not state or code.
- Compensating for extreme network latency/packet loss on poor Wi-Fi networks (standard Link fallback behaviors apply).
