# 🔭 Vantage: Spec for chooseBy

## 👤 User Story
"As a Live Coder, I want to deterministically select between multiple pattern variations using an external control pattern, so that I can create structured, repeatable macro-changes over time instead of relying purely on random chance."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus has robust randomized selection via `choose` and `wchoose`, and Markov chains for probabilistic sequences. However, it lacks a deterministic way to multiplex or switch between different pattern streams based on another signal. In professional live coding and composition, structure is often driven by a slow-moving control pattern. Without `chooseBy`, users cannot easily dictate structural changes (e.g. play pattern A during one cycle, and pattern B during another) using the language's native pattern composition. By introducing an external selector pattern to drive choice, we unlock deterministic, structured composition, elevating Orpheus from a probabilistic generator to a precise sequencing tool. Complexity is a cost; utility is revenue.

## 🎯 Metric Definition
- **Success** = The `chooseBy` builtin allows a selector pattern to index into a list of target patterns. The resulting pattern correctly evaluates to the events of the selected target at any given time, seamlessly switching when the selector pattern's value changes, with zero panics or out-of-bounds indexing errors.

## 🔍 Gap Analysis
- **Current State (Orpheus):** `choose` and `wchoose` provide probabilistic selection, but their output is driven by an internal random number generator (salted by site and time). There is no way to inject an explicit control pattern to drive the choice.
- **Competitors (TidalCycles):** TidalCycles offers `chooseBy` to handle external pattern selection.
- **The Gap:** Orpheus needs the `chooseBy` implementation in its pattern evaluator, complementing the existing stochastic choice functions with a purely deterministic, pattern-driven multiplexer.

## ✅ Acceptance Criteria
- Must introduce a `chooseBy` pattern transformation builtin.
- Must accept a selector pattern and a list (or arguments) of target patterns.
- Must evaluate the selector pattern at the queried time and use its value to index into the target patterns.
- Must gracefully handle out-of-bounds values from the selector pattern without crashing.
- Must not advance any internal random state or localized cycle counters for unselected patterns.

## 🚫 Out of Scope
- Probabilistic or weighted selection (this is already handled by `wchoose`).
- Multi-dimensional matrix selection.
