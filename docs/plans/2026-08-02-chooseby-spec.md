# 🔭 Vantage: Spec for chooseBy

## 👤 User Story
As a Live Coder, I want to selectively choose among multiple target patterns based on the current value of an integer index pattern (`chooseBy`), so that I can programmatically trigger different variations or sections using a single sequence, enabling explicit compositional structure instead of just random probabilities.

## 🎯 The "So What?"
**What business problem does this solve?**
Currently, Orpheus has probabilistic sequence selection (`pchoose`, `wpchoose`, `randcat`, `markov`) which creates organic, unpredictable variations. However, it lacks a deterministic multiplexer. Without a way to explicitly route an index to a pattern, users cannot write structured arrangements compactly. This forces users to write out long, manually concatenated sequences or rely on unpredictable randomness for variety. Adding `chooseBy` enables deterministic, pattern-driven composition, directly fulfilling a missing Tidal parity requirement.

## 📏 Metric Definition
Success = `chooseBy(index_pattern, p0, p1, ...)` allows users to deterministically select patterns from a list based on the zero-indexed integer output of `index_pattern`. The structure of the output should strictly match the timing of the `index_pattern`.

## 🔍 Gap Analysis
Looking at the current `orpheus-lang` builtins, we have:
- `pchoose` / `wpchoose`: Uniform or weighted random choice per slot.
- `randcat` / `wrandcat`: Random choice per cycle.
- `markov`: State-based random choice.

The parity roadmap explicitly notes a remaining gap under `pchoose`: `remaining: chooseBy driven by an external selector pattern`.

## ✅ Acceptance Criteria
- **Syntax**: Must expose a `chooseBy(index, p0, p1, ...)` builtin function.
- **Arguments**: Takes an integer control pattern (`index`) and at least one target pattern (`p0`). Target patterns can be evaluated.
- **Semantics**:
  - The timing and structure of the result is driven by the `index` pattern.
  - For each event in `index` with value `i`, the resulting pattern plays the events of target pattern `i` that fall within that event's timespan.
  - **Wrapping**: If the index `i` exceeds the number of target patterns `N`, it must wrap around. Negative indices should wrap backward or take the absolute value before wrapping.
- **Robustness**:
  - Must handle empty `index` patterns gracefully (yielding silence).

## 🚫 Out of Scope
- Audio-rate index modulation. The index pattern is evaluated at query time (block/event rate), not as a continuous DSP signal.
