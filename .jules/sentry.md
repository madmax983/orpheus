## 2024-03-24 - [Coverage Gap in `eval.rs` Pattern Evaluation]
**Learning:** The evaluator logic for checking pattern types (e.g. `eval_structural_pattern`, `unsupported_pattern_item_error`) and verifying explicit-time restrictions (like `stream(...)` vs implicit `seq(...)`) contained many robust error messages that were completely untested.
**Action:** Use `cargo llvm-cov` to identify missing error-handling branch coverage. Write targeted unit tests utilizing `eval_module` that intentionally violate structural grammar and explicit-time restrictions to assert that the exact intended `EvalError` messages are produced.

## $(date +%Y-%m-%d) - repl & export test coverage gap
**Learning:** Found testing gaps for early returns resulting from limits (e.g. cycle count 0) and terminal interaction helper loops mapping reader lines to the session state machine.
**Action:** When working on CLI interaction boundaries, try mocking `BufRead` with `Cursor` structures to unit-test logic branches without doing extensive full-system integration tests.
