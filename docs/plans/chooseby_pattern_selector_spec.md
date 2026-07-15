# 🔭 Vantage: Spec for chooseBy Pattern Selector

## 👤 User Story
"As a Live Coder, I want to deterministically select elements from a list of patterns using an external numerical control pattern, so that I can drive complex melodic or rhythmic variations explicitly from a master sequence rather than relying solely on random probability."

## ❓ The "So What?" (Business Problem)
Currently, Orpheus provides `choose` and `wchoose` for site-salted random selection. However, true generative music requires deterministic cross-referencing—using one pattern (like a slow counter or a Euclidean rhythm) to index into a pool of options. Without `chooseBy`, users are forced to write deeply nested `every`/`when` statements to achieve deterministic structural changes. Complexity is a cost; utility is revenue. `chooseBy` bridges this gap, allowing a single control pattern to act as a "macro sequencer" for other patterns. This provides users with precise structural control, making the language more expressive and reducing boilerplate.

## 🎯 Metric Definition
- **Success** = Users can evaluate `chooseBy(selector_pattern, pat1, pat2, pat3)`, where the `selector_pattern` deterministically indexes into the list of options at runtime, preserving the exact rational timing of the selector events, with zero regressions in the parser or evaluator test suites.

## 🔍 Gap Analysis
- **Current State (Orpheus):** Options can only be selected randomly (`choose`, `wchoose`, `randcat`). There is no built-in primitive for using a `Pattern<Number>` to directly index into a list of `Pattern<T>`.
- **Competitors (TidalCycles):** TidalCycles natively supports `chooseBy` (and similar select functions), allowing external integer patterns to select from lists of patterns.
- **The Gap:** The `parity-roadmap.md` explicitly lists `chooseBy driven by an external selector pattern` as the remaining gap in the probabilistic/selection pattern sequencing section.

## ✅ Acceptance Criteria
- Must introduce a new `chooseBy` or equivalent builtin function that accepts a selector pattern and a set of option patterns.
- Must accept a `Pattern<Number>` as the `selector`.
- Must safely map the `selector` values into the bounds of the provided options (e.g., via modulo arithmetic: `value.floor() as usize % options.len()`).
- The structure (timing and events) of the output pattern must be driven by the active segments of the selected option, conforming to the structural intersection rules.
- Must handle edge cases, such as an empty list of options, by returning silence rather than panicking.

## 🚫 Out of Scope
- Implementing first-class list data structures (`[a, b, c]`) at the language level if they do not yet exist (using variadic arguments `chooseBy(selector, a, b, c)` is acceptable for Phase 1).
- String-based or dictionary-based key lookup selectors.
