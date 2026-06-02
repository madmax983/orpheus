# 🔭 Vantage: Spec for Collaborative Multiplayer

## 👤 User Story
"As a Live Coder and Performer, I want to connect to a shared Orpheus session over a network with other musicians, so that we can write code, sequence patterns, and perform together in real-time within the same audio environment, without having to crowd around a single laptop."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus is a strictly local, single-player experience. Live coding, however, thrives on collaboration and ensemble performance (e.g., algoraves). If Orpheus cannot support multiple musicians performing together in the same virtual space, it limits its viability for modern electronic music collectives and remote collaboration. Complexity is a cost; utility is revenue. Transforming Orpheus into a multiplayer environment directly taps into the social and collaborative core of live coding, making it a platform for shared creation rather than an isolated tool. It differentiates Orpheus from traditional, single-user DAWs.

## 🎯 Metric Definition
- **Success** = Multiple users (up to 4) can connect to a central Orpheus host over a local network or the internet. Code executed by any user instantly synchronizes state across all clients with <50ms network latency. The audio engine on the host machine flawlessly renders the combined patterns without dropouts, and all clients receive visual feedback of the updated state (e.g., in the TUI).

## 🔍 Gap Analysis
- **Current State (Orpheus):** State and execution are entirely localized to the process running on a single machine. There is no concept of networking or shared session state.
- **Competitors (Troop, Estuary, Flok, TidalCycles):** Tools like Troop and Flok allow collaborative text editing for SuperCollider/TidalCycles. Estuary is web-based and natively collaborative.
- **The Gap:** Orpheus needs a network layer that synchronizes pattern definitions, environment variables, and execution commands between a host and multiple clients, effectively turning the Orpheus engine into a multi-client server.

## ✅ Acceptance Criteria
- Must introduce a network protocol (e.g., via TCP/WebSockets) to allow clients to connect to an Orpheus host.
- Must synchronize the core environment state (variable bindings, pattern definitions) across all connected clients in real-time.
- Must allow any connected client to evaluate code and have it execute on the host's audio engine.
- Must resolve conflicting updates gracefully, ensuring all clients eventually converge on the same session state.
- Must provide clear feedback in the TUI indicating connection status and which user executed a given command.

## 🚫 Out of Scope
- Streaming real-time audio back to the clients. Phase 1 assumes the host is outputting audio to a PA system or stream, and clients are merely sending code.
- Granular, Google Docs-style real-time text editing of the same line. Phase 1 focuses on block/expression evaluation syncing.
