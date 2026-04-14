# 🔭 Vantage: Spec for Collaborative Multiplayer Sessions

## 👤 User Story
"As a Live Coder, I want to collaborate in real-time with other performers over a network in a shared session, so that we can compose and perform algoraves together simultaneously without stepping on each other's toes."

## ❓ The "So What?" (Business Problem)
Live coding performances are often collaborative, but Orpheus is currently designed as a single-player experience running locally on one machine. When performers collaborate, they typically have to physically sit next to each other, stream screens, or rely on complex external sync tools (like Ableton Link, which just syncs clocks, not state or code). This fragmentation introduces severe friction and limits the potential for remote jam sessions and distributed band configurations. Complexity is a cost; enabling seamless remote state synchronization transforms Orpheus from an isolated tool into an extensible platform for modern digital ensembles, multiplying the utility and the potential user base.

## 🎯 Metric Definition
- **Success** = Users can join a shared session over a network using a simple command (e.g., `:connect <ip>`). When user A evaluates a pattern binding, user B's TUI updates instantly, and the audio state evaluates simultaneously on both clients with <20ms jitter relative to the downbeat, all without causing crashes or race conditions.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Execution, session state, and TUI display are all strictly local and single-player.
- **Competitors (Troop, Estuary, Flok):** Tools like Troop and Flok are built explicitly for collaborative live coding (often wrapping SuperCollider or TidalCycles). Estuary enables web-based ensemble performances.
- **The Gap:** Orpheus lacks any concept of network awareness, shared CRDTs (Conflict-free Replicated Data Types) for code editing, or a master/slave clock mechanism to negotiate cycle boundaries across different machines.

## ✅ Acceptance Criteria
- Must introduce a network sync layer to establish a "host" and "client" relationship between multiple Orpheus instances.
- Must synchronize the TUI editor state (e.g., cursor positions, text) using an operational transformation (OT) or CRDT approach.
- Must synchronize the session state so that variables/bindings updated by one user are immediately accessible in the scope of other users.
- Must negotiate a shared time cycle clock across the network, adjusting for latency to ensure events trigger simultaneously on both machines' DSP engines.
- Must handle network dropouts gracefully without panicking the audio thread.

## 🚫 Out of Scope
- Streaming audio over the network. Each client renders its own audio locally based on the shared code state.
- Public matchmaking or server hosting (e.g., an Orpheus cloud server). Phase 1 is strictly peer-to-peer or local LAN connections.
- Fine-grained permission systems (e.g., "User A can only edit track 1"). In Phase 1, everyone has full read/write access to the session.
