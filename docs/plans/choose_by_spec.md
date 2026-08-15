# 🔭 Vantage: Spec for choose_by

## 👤 User Story
"As a Live Coder, I want to use an external pattern to select elements from a list, so that I can decouple the rhythm or control signal of selection from the actual musical events, allowing complex external sequencing of a static sample or synth bank."

## ❓ The "So What?" (Business Problem)
Currently, the codebase allows random choices via random combinators, but none of these allow deterministic, index-based selection driven by a separate, possibly rhythmic or algorithmic, control pattern. If a user wants to cycle through a list of choices based on an explicit index sequence, they cannot easily do so. By adding a deterministic choose-by combinator, we allow complete decoupling of content from selection sequence, vastly increasing the compositional power of the pattern language. Complexity is a cost; utility is revenue. This is a high-utility primitive for explicit sequence manipulation.

## 🎯 Metric Definition
- **Success** = The new combinator plays the choice at the designated index for each event in the selector pattern.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Has probabilistic combinators. These are driven by randomness.
- **Competitors (TidalCycles):** TidalCycles has `chooseBy`, which uses a continuous or discrete pattern of numbers to index into a list of patterns.
- **The Gap:** Orpheus lacks deterministic, pattern-driven indexing into a set of choices.

## ✅ Acceptance Criteria
- Must introduce a deterministic choose-by combinator.
- Must accept a numeric selector pattern as the first argument, and a list of pattern choices as subsequent arguments.
- Must use the value of the selector pattern to index into the choices.
- The index should wrap around (modulo) the number of choices.
- Timing of events should strictly follow the timing of the selector pattern.

## 🚫 Out of Scope
- 2D matrix selection (Phase 2).
- Interpolation between choices (e.g. crossfading if index is a float - it should just cast/truncate to int).