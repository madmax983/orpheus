# 🔭 Vantage: Spec for Sidechain Compression

## 👤 User Story
"As an Electronic Musician and Live Coder, I want to route the audio output of a percussive pattern (like a kick drum) into a sidechain bus that ducks the volume of my bass and pad layers, so that I can achieve the rhythmic 'pumping' effect essential for clear mixes in modern dance music."

## ❓ The "So What?" (Business Problem)
Without sidechain compression, heavy rhythmic elements clash with sustained basslines and dense pads, resulting in a muddy, indistinct mix. In Orpheus, patterns currently generate audio independently. However, professional music production requires elements to interact dynamically. Sidechain ducking is not merely a mixing utility—it is a foundational creative tool for genres like Techno, House, and Lo-Fi. Adding this capability transforms Orpheus's DSP engine from a simple layered playback system into a cohesive, professional-grade mix environment. Utility is revenue; giving artists the ability to glue their mix together dynamically dramatically increases the software's value.

## 🎯 Metric Definition
- **Success** = Users can define a sidechain relationship between two or more tracks (e.g., ducking `bass` using `kick`) where the target track's gain is reduced by a specified ratio in <2ms of the source transient, recovering smoothly based on attack/release parameters, with zero added round-trip audio latency or thread-blocking allocations.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Audio routing flows linearly from track generation to the master output or through isolated, shared effects buses (like reverb/delay). There is no inter-track dynamic routing or envelope following to allow one track to modulate another.
- **Competitors (Ableton Live, Renoise, SuperCollider):** Ableton has a native Compressor with a dedicated sidechain routing UI. Renoise uses a "Signal Follower" to modulate parameters. SuperCollider can achieve this via standard bus routing and `Compander` UGens.
- **The Gap:** Orpheus lacks an audio-rate envelope follower DSP block and the necessary internal routing infrastructure to allow the amplitude of one track's buffer to modulate the gain node of a different track during the same audio processing block.

## ✅ Acceptance Criteria
- Must introduce a DSP node for an Envelope Follower / Ducking Compressor that can accept a secondary "sidechain" audio input.
- Must introduce pattern language syntax (e.g., `|> duck("kick", threshold, ratio)`) allowing a track to specify which layer controls its amplitude.
- Must handle the internal DSP graph routing to supply the sidechain signal to the target track before the master mix stage.
- Must prevent circular routing feedback loops (e.g., `track A` sidechains `track B`, which sidechains `track A`).
- Must operate allocation-free and lock-free on the real-time audio thread.

## 🚫 Out of Scope
- Multi-band sidechain compression.
- Lookahead compression (which introduces latency). Phase 1 will react immediately to the audio signal.
- Modulating parameters other than volume/gain (e.g., sidechain filter cutoff). Phase 1 focuses strictly on amplitude ducking.
