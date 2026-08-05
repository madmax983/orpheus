# 🔭 Vantage: Spec for chooseBy

## 👤 User Story
As a Live Coder, I want to use one pattern to select elements from a list of other patterns, so that I can drive structural changes and algorithmic sequences using a separate, controllable numeric source.

## ❓ The "So What?"
Currently, Orpheus has random choice functions like `pchoose`, but lacks deterministic, patterned control over which option is selected from a list. Without this, building structured variations requires verbose sequences. By introducing `chooseBy`, we separate the control sequence from the content options, allowing artists to compose complex variations easily. Complexity is a cost. Utility is a revenue.

## 🎯 Metric Definition
Success = `chooseBy` accurately indexes into the provided list of patterns based on the external selector pattern, completing evaluations without dropping events.

## 🔍 Gap Analysis
- Current State (Orpheus): We have `pchoose` and `wpchoose` for random choice between patterns, but lack a mechanism to explicitly select between pattern arguments driven by an external selector pattern.
- The Gap: Orpheus needs a deterministic combinator that takes a selector pattern and a list of pattern options, playing the option at the current index provided by the selector pattern.

## ✅ Acceptance Criteria
1) Must introduce a `chooseBy` function that accepts a selector pattern and a list of target patterns.
2) Must evaluate the selector pattern and use its values to index into the list of target patterns.
3) Must wrap the index if the selector value is larger than the number of provided target patterns.
4) Must preserve the rhythm and timing of the target patterns.

## 🚫 Out of Scope
1) Probabilistic or random selection.
2) Advanced matrix routing.