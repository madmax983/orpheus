# 🔭 Vantage: Spec for Song-Level Structure

## 👤 User Story
"As a Composer, I want to sequence multiple pattern sections along a timeline, so that I can arrange full, structured songs rather than just playing infinite looping cycles."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus excels at creating complex, infinite looping patterns, which is perfect for live-coding and jamming. However, a significant portion of music creation involves arranging discrete sections (e.g., Intro, Verse, Chorus, Outro) into a final, composed piece. Without a way to sequence sections over time, users are forced to manually trigger code changes live or write overly complex, monolithic patterns using `every` and `when` logic that quickly becomes unmaintainable. Complexity is a cost; utility is revenue. By adding song-level sequencing, we expand Orpheus from a live performance tool into a full composition environment, allowing users to build and export complete, durable musical artifacts.

## 🎯 Metric Definition
- **Success** = The engine can seamlessly transition between distinct pattern sections at exact cycle boundaries over a defined timeline, resulting in a deterministic arrangement without audio clicks, dropouts, or dropped events on the real-time thread.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Patterns loop indefinitely. The only way to create sections is manual code evaluation or complex conditional logic.
- **Competitors (TidalCycles, Sonic Pi):** Sonic Pi allows sequential block execution natively. TidalCycles has some sequence arrangement but is primarily loop-centric.
- **The Gap:** Orpheus needs a declarative, time-bounded section sequencer that evaluates different AST branches based on the absolute cycle counter.

## ✅ Acceptance Criteria
- Must introduce a `section(pattern, cycle_duration)` primitive.
- Must introduce a `seq_sections(section1, section2, ...)` primitive.
- The active section must swap deterministically at exact cycle boundaries.

## 🚫 Out of Scope
- Interactive, non-linear scene launching (Phase 2).
