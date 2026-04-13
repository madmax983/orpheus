# 🔭 Vantage: Spec for Swing and Groove Modeling

## 👤 User Story
"As a Beatmaker and Live Coder, I want to apply customizable swing, micro-timing shifts, and groove templates to my rigidly quantized patterns, so that my music feels human, organic, and physically engaging rather than stiff and robotic."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus's exact-rational temporal model divides time perfectly. A 16-step hi-hat pattern lands on mathematically flawless subdivisions. While this is great for algorithmic precision, it is deadly for actual dance music, hip-hop, or any genre where the "pocket" or "swing" is what makes the music feel good. If a user cannot nudge off-beats or apply classic groove feels (like the MPC60 or SP1200 swing), the output sounds sterile. Complexity is a cost, but groove is the product. A rigid sequencer is only a toy; a sequencer that can push and pull time is a musical instrument. Introducing swing and micro-timing exponentially increases the musical utility and professional viability of Orpheus.

## 🎯 Metric Definition
- **Success** = Users can apply a global or per-pattern swing amount (e.g., a percentage from 50% straight to 75% hard swing) or a named groove template. The timing shifts must recalculate event start times accurately within the pattern engine before querying, without introducing any jitter, allocations, or audio artifacts on the real-time audio thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Events are strictly bound to their rational subdivisions. There is no built-in primitive for systematically shifting the `part` and `whole` time spans of off-beat events.
- **Competitors (Ableton Live, Logic Pro, TidalCycles):** All modern DAWs feature extensive "Groove Pools" and swing percentages. TidalCycles has `nudge` and `swingBy` which are frequently used to humanize algorithmic beats.
- **The Gap:** Orpheus needs a pattern transformation primitive (a function `Pattern<T> -> Pattern<T>`) that delays or advances specific subdivisions within the cycle, acting as a temporal warping layer just before events hit the DSP scheduler.

## ✅ Acceptance Criteria
- Must introduce a `swing(amount)` pattern transformation, where `amount` is a number (or a patterned number) representing the percentage of delay applied to the even 16th notes (or 8th notes depending on the implementation scale).
- Must introduce a `nudge(time)` pattern transformation to shift the entire pattern forward or backward by a specific rational or absolute time amount.
- Must ensure that applying `swing` does not break the `whole` vs `part` relationship for events that cross cycle boundaries.
- Must allow swing to be applied per-pattern via the pipe operator (e.g., `hats |> swing(0.6)`).
- Must recalculate the temporal query correctly so that the audio thread receives events at the right time without needing to know about "swing" itself (the transformation happens purely in the pattern domain).

## 🚫 Out of Scope
- Extracting groove templates from audio files (Audio-to-MIDI groove extraction). Phase 1 is purely generative mathematical swing.
- Real-time performance recording with humanization (recording unquantized MIDI input). Phase 1 focuses on applying groove to sequenced code.
- Variable tempo curves (e.g., ritardando or accelerando over multiple bars). Phase 1 focuses on intra-cycle micro-timing.
