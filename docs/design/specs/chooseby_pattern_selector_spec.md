# 🔭 Vantage: Spec for chooseBy Pattern Selector

## 👤 User Story
"As a Live Coder, I want to use an external pattern to drive the selection of a list of patterns, so that I can decouple the rhythm of my pattern selection from the content of the patterns themselves."

## The "So What?" (Business Problem)
Currently, Orpheus has random pattern choice (`choose`) and probabilistic weighting (`wchoose`), but lacks deterministic, pattern-driven selection. Without an external selector pattern, composers cannot programmatically cycle through patterns based on complex rhythms or external data streams. This limits the expressive power of the pattern language and breaks feature parity with TidalCycles. Adding `chooseBy` closes this gap, empowering users to create intricate, algorithmic song structures that are predictable and reproducible.

## Metric Definition
- **Success** = Users can select between multiple patterns using an external control pattern driving the selection.

## Gap Analysis
- **Current State:** The language supports `choose` (random choice) and `wchoose` (weighted random choice), but no deterministic external selection.
- **Competitors:** TidalCycles has full support for `chooseBy`.
- **The Gap:** A mechanism to select from a list of patterns using an external control pattern.

## ✅ Acceptance Criteria
- Must introduce a `chooseBy` transform that takes a selector pattern and a list of candidate patterns.
- The output pattern must draw events from the candidate pattern corresponding to the value of the selector pattern at any given time.
- Must safely handle cases where the selector pattern evaluates to a value outside the bounds of the candidate list.

## 🚫 Out of Scope
- Multi-dimensional matrix selection.
