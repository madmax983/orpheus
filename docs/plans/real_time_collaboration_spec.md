# 🔭 Vantage: Spec for Real-time Collaboration

## 👤 User Story
As a live-coding musician, I want to seamlessly connect my Orpheus instance with other musicians over a network, so that we can compose, edit patterns, and perform together in a shared session without manually passing files back and forth.

## 🏢 So What? (Business Problem)
Live coding is often perceived as a solitary, isolated performance art. By introducing real-time multiplayer collaboration natively into the engine, Orpheus transforms from a single-player compositional tool into a collaborative ecosystem. This unlocks remote jam sessions, live band performances, and educational workshops, driving growth through network effects (musicians invite their peers). While network synchronization introduces technical complexity, the utility of a shared, natively synced environment provides massive value that external tools cannot replicate.

## 📈 Metric Definition
- **Success:** Time synchronization drift is < 5ms between connected instances over a 1-hour session.
- **Success:** Simultaneous edits to the shared pattern state resolve deterministically without crashing the DSP engine or dropping audio frames.
- **Success:** 90% of users can establish a peer-to-peer connection without needing to manually configure port-forwarding (e.g., via NAT traversal or a lightweight relay).

## 🔍 Gap Analysis
- **Current State:** Orpheus is entirely single-player. Collaboration requires manual copy-pasting of code or using generic tools like Git.
- **Market Standard:** Existing live-coding environments rely on third-party web-based text editors (like Flok or Troop) that synchronize text but are decoupled from the underlying audio engine's cycle clock.
- **The Gap:** Orpheus needs a built-in, zero-config collaboration primitive that synchronizes both the REPL state and the exact rational time boundaries natively, eliminating the friction of external text-sync tools.

## 🚫 Out of Scope
- Streaming raw audio data over the network (Phase 2 or out of scope; each client will execute the code and render audio locally).
- Complex role-based permissions (e.g., Admin vs. Read-Only). In Phase 1, all connected participants have full read/write access.
- In-app text or voice chat. Participants are expected to use existing platforms (Discord, Zoom) for communication.
