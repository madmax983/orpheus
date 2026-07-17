# 🔭 Vantage: Spec for chooseBy external selector pattern

## 👤 User Story
"As a Live Coder, I want a `chooseBy` pattern combinator that allows me to select from a list of patterns based on a continuous or discrete external selector pattern (like a sine wave, a LFO, or an array of indices), so that I can structurally orchestrate complex pattern variations over time without relying on purely random selection (`choose`) or manual typing."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus has probabilistic selection mechanisms like `choose`, `randcat`, and `wchoose` for selecting patterns. However, these are inherently stochastic. When a composer wants deterministic, evolving control over which pattern plays at any given time (e.g., using a slow `sine` wave to sweep through an array of drum patterns, or an external control signal to trigger specific chord progressions), there is no direct functional way to map a control signal to a choice of pattern. Without `chooseBy`, the composer is stuck manually writing out long sequences or relying on randomness. Complexity is a cost; utility is a revenue. By implementing `chooseBy`, we unlock deterministic modulation of musical structure, significantly increasing the compositional power of the Orpheus language and matching the advanced capabilities of systems like TidalCycles.

## Metric Definition
- **Success** = Users can evaluate expressions like `chooseBy(sine, [pattern1, pattern2, pattern3])` where the `sine` control pattern continuously selects between the patterns over a cycle, and the engine correctly queries the active sub-pattern without causing runtime errors or infinite loops.

## Gap Analysis
- **Current State (Orpheus):** Users can randomly select patterns (`choose`, `wchoose`), but cannot deterministically select sub-patterns using an external control signal or index pattern.
- **Competitors (TidalCycles):** TidalCycles has `chooseBy` which takes a pattern of floats (or indices) and a list of patterns, deterministically selecting the output based on the control value.
- **The Gap:** The `chooseBy` transform is explicitly marked as "remaining" in the parity roadmap under the probabilistic pattern sequencing capabilities, missing the deterministic "external selector" counterpart to `choose`.

## ✅ Acceptance Criteria
- Must implement the `chooseBy` function in the pattern runtime.
- Must accept a control pattern (yielding values like floats `0.0` to `1.0` or integers) and a list (or tuple) of target patterns.
- Must map the control pattern's values to indices of the target list, selecting the corresponding pattern for the duration of that control value's event.
- Must gracefully handle out-of-bounds control values (e.g., by clamping or wrapping indices).
- Must resolve the sub-patterns exactly as the `choose` family does, without breaking the natural global timeline of the selected patterns.

## 🚫 Out of Scope
- Expanding the behavior to handle nested multi-dimensional selections (e.g. matrices of patterns) in Phase 1.
- Replacing existing stochastic functions like `choose` and `wchoose` (they solve different use cases).
