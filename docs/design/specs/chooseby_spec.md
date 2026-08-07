# 🔭 Vantage: Spec for chooseBy

## 👤 User Story
"As a Live Coder, I want to use an external selector pattern to dynamically choose between multiple musical patterns, so that I can create deterministic, evolving sequences driven by other patterns rather than pure randomness."

## ❓ The "So What?"
Currently, Orpheus provides pchoose and wpchoose for selecting between patterns via site-salted randomness. While stochastic choices are useful, artists often need structured choices—where an external selector pattern dictates the selection from a list of patterns over time. By introducing chooseBy, we bridge the gap between strict loop automation and structural routing. Complexity is a cost; utility is a revenue. Enabling pattern-driven routing allows users to build highly dynamic macro-structures from simple micro-patterns, increasing the generative power of the platform without adding unpredictability.

## 🎯 Metric Definition
- **Success** = The selector pattern accurately and deterministically indexes into the provided pattern list, triggering the correct candidate events, while adding less than 1ms of evaluation overhead per cycle.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Supports pchoose and wpchoose for randomized, probability-based selection. As noted in the parity roadmap, the remaining gap is a chooseBy combinator driven by an external selector pattern.
- **The Gap:** Orpheus requires a chooseBy function that accepts an external selector pattern and a set of candidate patterns, mapping the selector's value to the corresponding candidate at any given time.

## ✅ Acceptance Criteria
- Must introduce a chooseBy function that accepts an external selector pattern and a list of candidate patterns.
- Must correctly map the selector pattern's values to the corresponding candidate pattern.
- Must ensure that evaluating chooseBy is completely deterministic and perfectly repeatable based on the selector pattern's values.
- Must be fully composable with existing pattern modifiers and time transformations.

## 🚫 Out of Scope
- Audio-rate crossfading between candidate patterns. chooseBy operates strictly at the structural/event level, not as a DSP signal mixer.