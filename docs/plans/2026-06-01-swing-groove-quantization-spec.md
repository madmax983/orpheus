# 🔭 Vantage: Spec for Swing & Groove Quantization

## 👤 User Story
"As an Electronic Musician, I want to apply swing and groove quantization to my rigid, perfectly-timed patterns, so that my drums and basslines have a more human, syncopated, and danceable feel."

## ❓ The "So What?" (Business Problem)
Computer-generated music, especially when built on exact rational timing like Orpheus, is perfectly rigid. While perfect timing is mathematically sound, it is musically sterile. Most popular electronic music—from House to Hip-Hop—relies heavily on "swing" (delaying the off-beats) to create a groove that makes people want to dance. If Orpheus only produces perfectly quantized audio, it will sound robotic and lifeless compared to modern DAWs like Ableton Live or MPC hardware, which are famous for their groove engines. Complexity is a cost; utility is revenue. Adding a first-class swing and groove system transforms Orpheus from a rigid algorithmic sequencer into a musically expressive instrument, dramatically increasing its appeal and usability for producing actual dance music.

## 🎯 Metric Definition
- **Success** = Users can apply a `swing(amount)` transformation to any pattern, which delays the even-numbered 16th notes (or chosen subdivision) by a precise temporal percentage, without causing missed events, duplicate triggers, or breaking cycle boundary synchronization.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Patterns are evaluated on an exact rational timeline. Events fall exactly on their mathematical grid divisions. While users can manually shift events (e.g., `shift(amt)`), there is no native pattern transform that automatically identifies and delays off-beats to create swing.
- **Competitors (Ableton Live, MPC, TidalCycles):** MPCs are legendary for their swing (e.g., 50-75% swing). Ableton Live has a comprehensive Groove Pool. TidalCycles has the `swingBy` and `nudge` functions to achieve this.
- **The Gap:** Orpheus lacks a dedicated pattern transform (e.g., `PatternRuntime::Swing`) that applies a localized temporal distortion to off-beats within the exact rational timeline.

## ✅ Acceptance Criteria
- Must introduce a pattern transform (e.g., `|> swing(amount)`) in the language.
- Must support specifying the subdivision grid for the swing (e.g., 16th notes by default, but configurable).
- Must seamlessly shift the onset and duration of affected events in the pattern stream based on the `amount` (e.g., an amount of `0.1` pushes the off-beat 10% later into the grid).
- Must compose correctly with all other pattern transforms (like `fast`, `slow`, `euclid`) when piped together.
- Must not drop events that get swung across a query boundary; the query semantics must correctly resolve the time-shifted events.

## 🚫 Out of Scope
- Extracting groove templates from external audio files (Audio-to-Groove). Phase 1 is algorithmic swing based on a fixed grid and percentage.
- Micro-timing humanization (randomized micro-shifts). Swing is a deterministic, repeating groove; randomized humanization is a separate feature.
