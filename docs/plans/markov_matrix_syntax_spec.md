# 🔭 Vantage: Spec for Markov Matrix Syntax

## 👤 User Story
"As a Live Coder, I want to express Markov chain transitions using a concise list or matrix syntax, so that I can easily build complex generative and probabilistic musical sequences without manually typing out every interleaved state and weight argument."

## ❓ The "So What?" (Business Problem)
Generative music relies heavily on probabilistic state transitions, often modeled as Markov chains. Currently, Orpheus's `markov()` function requires a flat, unwieldy sequence of `k * (k+1)` arguments (states interleaved with all possible outgoing transition weights). This is hostile to human readers and error-prone to type live. It makes authoring anything beyond a 2-state chain practically impossible during a performance. Complexity is a cost; utility is revenue. Introducing a dedicated list or matrix syntax for `markovPat` (similar to TidalCycles) drastically reduces the cognitive load and keystrokes required to express probabilistic sequences, transforming a mathematical concept into an accessible, playable musical tool.

## 🎯 Metric Definition
- **Success** = Users can define a 4-state Markov chain using a nested list or matrix literal syntax instead of a flat list of 20 arguments, and the engine correctly parses and evaluates this syntax into the existing `markov` runtime representation with zero runtime overhead compared to the flat argument approach.

## 🔍 Gap Analysis
- **Current State (Orpheus):** The `markov` builtin takes a variable number of arguments and expects them to be strictly ordered: state 0, weights out of state 0... state 1, weights out of state 1... No list/array syntax exists in the language yet.
- **Competitors (TidalCycles):** TidalCycles uses `markovPat` which accepts a clear list of lists/tuples mapping states to transition probabilities.
- **The Gap:** Orpheus lacks the grammatical syntax to group arguments into lists or matrices. A new language-level construct (like square brackets `[]` for lists) is required to group the transition weights logically with their origin states.

## ✅ Acceptance Criteria
- Must introduce a list or array syntax (e.g., `[a, b, c]`) to the Orpheus parser and grammar.
- Must implement a new builtin (or adapt the existing `markov`) that accepts this nested structure.
- Must correctly lower this new syntax into the existing `PatternRuntime::Markov` data structure.
- Must not break the existing flat `markov()` function signature if backward compatibility is required (or provide a clean migration path).
- Must include comprehensive unit tests validating the parser and evaluator for the new syntax.

## 🚫 Out of Scope
- Implementing higher-order Markov chains (e.g., chains that remember the last N states). Phase 1 only targets first-order Markov chains by improving the syntax.
- Visual matrix editors in the TUI. This is strictly a language syntax feature.
