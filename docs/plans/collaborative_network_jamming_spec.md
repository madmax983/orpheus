# 🔭 Vantage: Spec for Collaborative Network Jamming

## 👤 User Story
"As a Band Member, I want to connect to a shared Orpheus session over a network, so that multiple people can livecode different tracks simultaneously on the same synchronized audio engine."

## ❓ The "So What?" (Business Problem)
Live coding is traditionally a solitary activity. Ensembles either have to manually sync clocks (like Ableton Link) and mix separate audio outputs, or pass a single keyboard around. A true network-jamming protocol allows collaborative composition in real-time. Complexity is a cost, but isolation is a limit. A native multiplayer mode turns the software from a solo instrument into a virtual band practice space.

## 🎯 Metric Definition
- **Success** = Multiple clients can connect to a headless host, evaluate code independently, and hear the synchronized audio output with less than 20ms jitter on network clock sync.

## 🔍 Gap Analysis
- **Current State:** Orpheus is strictly single-player; the REPL mutates a local engine state.
- **Competitors:** Troop (for FoxDot/Tidal) provides collaborative text editing. Estuary provides web-based collaboration. Flok provides web-based text syncing.
- **The Gap:** Orpheus needs a native client-server architecture where a host runs the audio DSP and state, and remote clients send AST fragments or evaluated events over UDP/TCP to be merged into the host's cycle.

## ✅ Acceptance Criteria
- Must implement a headless server mode (`orpheus --host`).
- Must implement a client mode (`orpheus --connect <IP>`).
- Must synchronize the rational time clock between host and clients.
- Must allow clients to evaluate code that updates specific tracks on the host without blocking the audio thread.
- Must broadcast track state changes (e.g., a new pattern evaluated by Client A) to Client B's UI.

## 🚫 Out of Scope
- Real-time collaborative text editing in the same file (Google Docs style). Each user has their own local REPL buffer.
- Streaming audio from host to client. Audio is generated locally at the host (or users must use a separate tool like Sonobus for audio transmission).