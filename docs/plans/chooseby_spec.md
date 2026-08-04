# Spec: chooseBy

👥 **User Story:**
As a livecoder, I want to select patterns or values from a list using an external selector pattern, so that I can create structured, repeatable variations driven by control signals rather than just random chance.

💡 **The "So What?":**
What business problem does this solve? Pure randomness (`choose`, `pchoose`) is hard to control in a structured composition. By allowing deterministic selection driven by a pattern, users gain predictable structure. Complexity is a cost, Utility is a revenue: this feature maximizes utility by enabling complex arrangements from simple, controllable inputs.

📏 **Metric Definition:**
Success = The `chooseBy` function correctly selects the target pattern based on the selector pattern's value at a given time without panicking, and query latency remains < 10ms for 99% of requests.

🕳️ **Gap Analysis:**
The `parity-roadmap.md` explicitly states: `remaining: chooseBy driven by an external selector pattern`. While the codebase currently provides `choose`, `wchoose`, `pchoose`, and `wpchoose` for random selection (implemented in `crates/orpheus-lang/src/builtins.rs`), it lacks a built-in function to deterministically select from a list of options based on the evaluation of an external selector pattern.

✅ **Acceptance Criteria:**
- Must accept a selector pattern as the first argument, followed by a list of target patterns or values.
- Must map the current value of the selector pattern to an index in the provided list.
- Must handle out-of-bounds selector values gracefully (e.g., by wrapping using modulo arithmetic).

🚫 **Out of Scope:**
- Multi-dimensional selection (matrices).
- Probabilistic selections within `chooseBy` (this is strictly deterministic).
