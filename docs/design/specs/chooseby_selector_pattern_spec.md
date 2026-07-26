# 🔭 Vantage: Spec for chooseBy Selector Pattern

👤 **User Story:**
As a live coder, I want to select elements from a list using a driving pattern, so that I can sequence choices deterministically rather than relying purely on random chance.

**The So What? (Business Problem):**
Currently, Orpheus has probabilistic choice via `choose`, but lacks deterministic, pattern-driven selection from a list of options. This forces users to write complex, nested `if/when` logic if they want to explicitly sequence specific variations or melodies. Adding `chooseBy` allows for elegant, index-based sequencing of patterns, improving compositional utility and reducing code verbosity. Complexity is a cost; utility is revenue.

**Metric Definition:**
Success = Users can successfully execute a `chooseBy` expression mapping a numerical pattern to a list of options without errors, and the pattern evaluation speed remains constant regardless of the list size.

**Gap Analysis:**
- **Competitors (TidalCycles):** TidalCycles supports deterministic list selection.
- **The Gap:** Orpheus has `choose` and `wchoose` for random selection, but as noted in the parity roadmap, "chooseBy driven by an external selector pattern" is the remaining gap for pattern transforms.

✅ **Acceptance Criteria:**
- Must introduce a `chooseBy` function to the pattern language.
- Must accept a numerical selector pattern and a list of pattern options.
- The output pattern must draw from the list of options based on the values of the selector pattern.

🚫 **Out of Scope:**
- Multi-dimensional matrix selection or Markov chain generation (handled separately).
