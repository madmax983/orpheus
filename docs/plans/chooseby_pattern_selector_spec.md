# 🔭 Vantage: Spec for chooseBy Pattern Selector

👤 **User Story:**
As a Live Coder, I want to use a numerical pattern to select which pattern plays from a list of options, so that I can decouple my musical structure from the actual musical content.

**The So What?**
Live coders often build complex textures by alternating between different patterns. Currently, choose and wchoose provide random selection, but users lack a deterministic, programmable way to switch between patterns using another pattern as a control signal. Adding chooseBy allows users to sequence structural changes explicitly, increasing the expressiveness and utility of the language.

**Metric Definition:**
Success = A user can evaluate a chooseBy expression where a selector dynamically selects the output pattern, with zero regressions in evaluation performance.

**Gap Analysis:**
- Current State (Orpheus): Users can randomly select patterns using choose and wchoose.
- The Gap: There is no deterministic combinator that routes an index from a selector pattern to a list of target patterns, forcing users into less modular compositions.

✅ **Acceptance Criteria:**
- Must provide a chooseBy function that accepts a selector pattern and a list of target patterns.
- Must route evaluation to the target pattern corresponding to the selector's current value.
- Must wrap the index (modulo) if the selector value exceeds the target list length.

🚫 **Out of Scope:**
- Multi-dimensional matrix selection.
- String-based or key-based selection.
