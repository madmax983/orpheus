# 🔭 Vantage: Spec for Global Send/Return Effect Buses

## 👤 User Story
"As a Mixing Engineer and Composer, I want to route multiple audio patterns to shared global effect buses (like a master reverb or a dub delay), so that I can create a cohesive acoustic space, manage complex effect chains centrally, and significantly reduce CPU load instead of duplicating heavy effects on every individual track."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus relies on insert effects. If a user wants reverb on their drums, bass, and synth, they must append `|> reverb(...)` to every single pattern. Reverb and delay algorithms are computationally expensive. Running ten independent reverbs can easily cause audio dropouts (buffer underruns). Additionally, from a musical perspective, sending multiple instruments to the *same* reverb glues the mix together, simulating a shared physical space. A live coding environment that cannot handle parallel send/return routing forces the user into computationally inefficient and acoustically disjointed mixes. Implementing shared buses upgrades Orpheus from a basic pattern sequencer to a viable mixing environment.

## 🎯 Metric Definition
- **Success** = Users can define named global effect buses (e.g., `bus("reverb") = input |> reverb(0.8)`) and send a configurable amount of any pattern's signal to that bus (e.g., `hats |> send("reverb", 0.5)`). The audio engine must process the buses in parallel and mix them with the master output without introducing phase issues or adding significant latency beyond the buffer size.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Effects are applied linearly as inserts via the pipe operator. There is no concept of auxiliary sends or a shared master effect chain.
- **Competitors (Ableton Live, SuperCollider, TidalCycles/SuperDirt):** DAWs rely heavily on Return Tracks (Aux Sends) for spatial effects. SuperCollider has extensive bus routing capabilities. SuperDirt (Tidal's backend) supports global effects like `# room` and `# size`.
- **The Gap:** Orpheus lacks an audio routing architecture that allows signals to split, be processed independently by a shared node, and summed back together at the master output.

## ✅ Acceptance Criteria
- Must introduce a `bus(name, effect_chain)` configuration to establish a persistent global effect bus.
- Must introduce a `send(bus_name, amount)` pattern transformation to route a percentage of a pattern's audio to the specified bus.
- Must allow `amount` to be dynamically patterned (e.g., `send("dub_delay", 0.0 0.8 0.0 0.0)` for a dub throw on the snare).
- Must process all buses in the DSP engine after the individual tracks, mixing their outputs into the master stereo bus.
- Must support querying the current routing state from the REPL/TUI.

## 🚫 Out of Scope
- Pre-fader sends. Phase 1 focuses exclusively on post-fader sends (the volume of the send depends on the track's main volume).
- Routing buses into other buses (feedback loops or complex matrix routing). Phase 1 only supports track-to-bus routing.
- Sidechain compression routing. While related to routing, sidechaining requires a different envelope-following mechanism. Phase 1 is strictly for send effects.
