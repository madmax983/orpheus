# 🔭 Vantage: Spec for Collaborative Network Sessions

## 👤 User Story
"As a Live Coder, I want to connect to a shared network session with other musicians over a local network or the internet, so that we can compose, sequence, and manipulate patterns together in the same real-time audio environment."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus is a solitary environment. Live coding is increasingly an ensemble art form, where multiple performers form bands (like in algoraves or laptop orchestras). If Orpheus cannot natively support multi-user collaboration, artists are forced to run completely separate instances, manually beat-match using external syncing tools like Ableton Link, and route audio physically between laptops. This friction stifles collaborative jamming and drastically limits Orpheus's appeal in a modern, connected performance context. Complexity is a cost; utility is revenue. Transforming Orpheus from a single-player sandbox into a multiplayer instrument unlocks a massive new demographic of bands, duos, and remote collaborators, shifting the paradigm of what a live coding session can be.

## 🎯 Metric Definition
- **Success** = Users can start or join a shared session using a command (e.g., `:network join "IP address"`), and any pattern evaluations or mixer changes executed by one user appear and take effect on all connected clients within <50ms over a LAN, with perfect musical cycle synchronization and without any audio artifacts or dropouts.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The REPL and TUI strictly modify local, single-player state. There is no concept of remote peers or shared state synchronization.
- **Competitors (Troop, Estuary, Extempore):** Troop and Estuary are explicitly built for collaborative live coding via text sharing in browsers. Extempore allows multi-user server access. TidalCycles relies on external synchronization (Ableton Link) but lacks native shared code environments, requiring third-party tools like Troop.
- **The Gap:** Orpheus needs a low-latency, peer-to-peer or client-server synchronization protocol that broadcasts code evaluations, mixer state changes, and beat synchronization across multiple clients natively within the terminal.

## ✅ Acceptance Criteria
- Must introduce commands to host a network session (`:network host [port]`) or join an existing session (`:network join [IP]`).
- Must guarantee precise beat and cycle synchronization between all connected peers to ensure patterns stay musically aligned.
- Must broadcast and apply REPL evaluations and pattern updates from any client to all other clients globally.
- Must display visual indicators in the TUI showing connected peers and identifying who is modifying which patterns.
- Must gracefully handle peer disconnections and reconnections without crashing the host or the session.

## 🚫 Out of Scope
- Streaming audio between clients. Phase 1 focuses purely on synchronizing REPL code evaluations and cycle timing (each client renders its own local audio).
- Granular, Google Docs-style real-time character typing synchronization. Phase 1 only synchronizes the final evaluated block when a user hits enter.
- Wide-area internet matchmaking servers or lobbies. Phase 1 relies on direct IP connection (LAN or VPN).
