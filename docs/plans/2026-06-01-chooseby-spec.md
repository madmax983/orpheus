# 🔭 Vantage: Spec for chooseBy

## 👤 **User Story:**
"As a Live Coder, I want to use an external selector pattern to deterministically choose between multiple pattern options, so that I can decouple my control logic from my musical content."

## ❓ **The "So What?":**
What business problem does this solve? Currently, Orpheus has random choice combinators (`choose`, `wchoose`, `pchoose`), but artists often need deterministic, data-driven selection where one pattern (like an LFO or integer sequence) drives the index of a list of patterns. By introducing `chooseBy`, we allow artists to separate their sequencing logic from their sound design, expanding the compositional utility of the platform. Complexity is a cost. Utility is a revenue.

## 🎯 **Metric Definition:**
Success = The `chooseBy` combinator deterministically selects the correct option based on the selector pattern's value, and evaluates seamlessly within the existing cycle architecture.

## 🔍 **Gap Analysis:**
- **Current State (Orpheus):** The language supports stochastic choices like `choose`, `wchoose`, and `pchoose`, but lacks an explicit index-driven selection combinator.
- **The Gap:** Orpheus needs a `chooseBy` combinator driven by an external selector pattern to enable deterministic pattern indexing.

## ✅ **Acceptance Criteria:**
- Must introduce a `chooseBy` function that accepts a selector pattern and multiple pattern options.
- Must use the values from the selector pattern to index and select from the given options deterministically.
- Must compose cleanly with other pattern transforms and modifiers.

## 🚫 **Out of Scope:**
- Stochastic or random selection mechanics.
- TUI enhancements for visualizing list indices.