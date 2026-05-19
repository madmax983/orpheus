# 🔭 Vantage: Spec for Song Structure Sequencing

## 👤 User Story
"As a Composer and Live Coder, I want to sequence multiple independent patterns or 'sections' into a cohesive timeline, so that I can arrange a full song with distinct parts (like verse, chorus, bridge) rather than being limited to endlessly looping a single monolithic stack."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus excels at creating endless, evolving loops through layered patterns. However, transitioning from a "jam" to a "finished track" requires macro-level structural changes over time. Live coding those changes by hand is error-prone and stressful during performance. If users cannot define a sequence of sections that automatically transition, they are blocked from using Orpheus to compose full, structured songs offline or define complex arrangements for playback. Complexity is a cost; utility is revenue. Bridging the gap between micro-loops and macro-structure multiplies Orpheus's utility, transforming it from a loop pedal into a complete composition engine.

## 🎯 Metric Definition
- **Success** = Users can define named pattern bindings (e.g., `verse`, `chorus`) and sequence them linearly over time using a new `seq_sections(...)` primitive. When evaluated, the audio output seamlessly transitions between sections at exact cycle boundaries without clicks, timing drift, or memory leaks, supporting a full 5-minute arrangement.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The pattern engine evaluates the current active stack indefinitely. There is no built-in notion of sequential arrangement or "song timeline."
- **Competitors (TidalCycles, Ableton Live):** TidalCycles uses `ur` and `seqP` for structure. Ableton Live provides the Arrangement View.
- **The Gap:** Orpheus lacks a language-level primitive to concatenate bounded lengths of different patterns into a single continuous master timeline.

## ✅ Acceptance Criteria
- Must introduce a `section(pattern, cycles)` primitive that bounds a pattern to a specific duration.
- Must introduce a `seq_sections(section1, section2, ...)` primitive that concatenates multiple sections linearly.
- Must accurately schedule and render the concatenated patterns across exact cycle boundaries.
- Must support looping the entire sequence if desired.
- Must execute all scheduling logic on the DSP thread (or pre-calculate efficiently) to ensure zero audio dropouts during transitions.
- Must allow using standard pattern transformations on the entire `seq_sections` output.

## 🚫 Out of Scope
- Granular crossfading between sections (Phase 1 will employ hard cuts at cycle boundaries).
- Complex conditional branching (e.g., "play verse 2 if random > 0.5"). Phase 1 is strictly linear sequencing.
- GUI-based arrangement timelines in the TUI.
