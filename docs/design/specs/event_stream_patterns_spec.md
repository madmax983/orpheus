# 🔭 Vantage: Spec for Event Stream Patterns (`stream(...)` and `at(...)`)

## 👤 User Story
"As a Composer, I want to define events that happen at specific, absolute times rather than looping endlessly, so that I can schedule one-shot sound effects, build narrative arcs, and create through-composed music."

## ❓ The "So What?" (Business Problem)
Orpheus's core primitive is the infinite, repeating cycle. This is fantastic for grooves and dance music but terrible for one-shot sound effects, long-form melodies, or traditional through-composed pieces. Without absolute time scheduling, users are forced to create massive, unwieldy cycles to simulate linearity, which is computationally inefficient and cognitively overwhelming. By introducing event streams, we unlock non-cyclic composition, making Orpheus suitable for film scoring, ambient soundscapes, and non-repetitive genres. Complexity is a cost; utility is revenue. Supporting linear time increases the utility of the language immensely.

## 📊 Gap Analysis
- **Market Standard (DAWs):** Absolute timelines are the default. Events happen once at a specific timecode.
- **Standard Libs (TidalCycles):** Tidal is famously hostile to non-cyclic time, requiring complex workarounds.
- **Our Gap:** We need syntax to escape the infinite loop and schedule isolated events on the global timeline.

## 🎯 Definition of Success
- **Metric:** Users can successfully schedule a pattern to play exactly once at a specific global cycle/time index, and the DSP engine plays it with sample-accurate precision.

## ✅ Acceptance Criteria
- Must introduce `at(time, pattern)` syntax to schedule a pattern to begin playing exactly at the specified absolute time.
- Must introduce `stream(...)` syntax to sequence multiple events chronologically without them repeating.
- Must correctly interleave these linear, one-shot events with the standard repeating cyclic events in the DSP scheduling queue.
- Must not drift; scheduling must be derived from the engine's internal time representation, not system wall-clock time.

## 🚫 Out of Scope
- A full timeline GUI editor (DAW style). The workflow remains code-first.
- Infinite generative streams that never terminate (for now, focus on bounded sequences).
