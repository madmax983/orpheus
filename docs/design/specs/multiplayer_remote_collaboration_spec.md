# 🔭 Vantage: Spec for Multiplayer Remote Collaboration

## 👤 User Story
"As a Live Coder, I want to share my Orpheus session state over the internet in real-time with other musicians, so that we can collaboratively compose, edit, and perform the same live coding session simultaneously from different physical locations."

## ❓ The "So What?" (Business Problem)
Music creation is increasingly a decentralized and collaborative process. While Orpheus allows individual users to compose and perform locally, it does not currently support remote jamming or co-writing. If two artists want to collaborate on a track, they are forced to share their screen via video calls, or manually pass `.ode` files back and forth via Git, which completely breaks the fluidity and real-time nature of live coding. Complexity is a cost; isolation is a limitation. By introducing real-time multiplayer editing—similar to Google Docs or Teletype for Atom—Orpheus becomes a powerful remote performance and education tool, drastically increasing its utility for bands, remote jam sessions, and online workshops.

## 🎯 Metric Definition
- **Success** = Users can connect to a shared online session where changes to the code or state from one user are reflected on all connected clients with <100ms latency. The audio engine evaluates these changes smoothly on the next cycle boundary without causing xruns or audio dropouts, ensuring all participants hear the same musical structure.

## 🔍 Gap Analysis
- **Current State (Orpheus):** A single-player, locally isolated environment. There is no concept of shared sessions or network synchronization of code and state.
- **Competitors (Troop for TidalCycles, Estuary, Strudel):** Estuary and Troop are specialized environments/tools specifically designed for multiplayer collaborative live coding. Strudel can be used collaboratively via external tools or specific instances.
- **The Gap:** Orpheus lacks a network synchronization layer for its Abstract Syntax Tree (AST) and REPL state, meaning it cannot natively support multiple simultaneous editors or shared execution states.

## ✅ Acceptance Criteria
- Must introduce a command to host a session (e.g., `:collab host`) and join a session (e.g., `:collab join <IP/URL>`).
- Must synchronize the text buffer and active pattern bindings across all connected clients in real-time.
- Must use Operational Transformation (OT) or Conflict-free Replicated Data Types (CRDTs) to handle concurrent edits to the same lines of code gracefully without losing data.
- Must ensure that evaluating code on one client updates the global state and triggers audio evaluation on all clients at the same time.
- Must visually distinguish the cursors or edits of different participants within the TUI.

## 🚫 Out of Scope
- Streaming the actual audio output over the internet (Phase 1 assumes each client renders the audio locally using the shared code).
- Real-time voice or video chat within the Orpheus TUI.
- Complex user permission levels or roles (e.g., read-only spectators). Phase 1 assumes all connected users have full read/write access.
