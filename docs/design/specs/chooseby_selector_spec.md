# 🔭 Vantage: Spec for chooseBy

👤 **User Story:** As a livecoder, I want to dynamically select elements from a list using an external control pattern, so that I can sequence sample choices deterministically rather than relying purely on randomness.

❓ **The "So What?" (Business Problem)**
Orpheus currently supports random selection (choose), but lacks deterministic, pattern-driven selection from a list of items. This limits users from writing complex, repeatable generative structures mapped to lists of samples or synths, forcing them to write tedious manual sequences.

🎯 **Metric Definition**
Success = Users can pipe a numerical pattern into chooseBy to predictably index an array of elements, with pattern evaluation time remaining under 2ms per cycle and zero audio dropouts.

🔍 **Gap Analysis**
TidalCycles has chooseBy. Max/MSP has index~. Orpheus currently only has choose and wchoose which are strictly probabilistic.

✅ **Acceptance Criteria:**
- Must accept a numeric control pattern as the first argument.
- Must map floating-point inputs correctly to list bounds (e.g., modulo math for out-of-bounds indices).
- Must evaluate deterministically without panicking on empty lists or NaN inputs.

🚫 **Out of Scope:**
- Multi-dimensional array or matrix selection.
- Audio-rate signal control of the index (control-rate pattern evaluation only).
