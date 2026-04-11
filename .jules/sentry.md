## 2024-03-24 - [Coverage Gap in `eval.rs` Pattern Evaluation]
**Learning:** The evaluator logic for checking pattern types (e.g. `eval_structural_pattern`, `unsupported_pattern_item_error`) and verifying explicit-time restrictions (like `stream(...)` vs implicit `seq(...)`) contained many robust error messages that were completely untested.
**Action:** Use `cargo llvm-cov` to identify missing error-handling branch coverage. Write targeted unit tests utilizing `eval_module` that intentionally violate structural grammar and explicit-time restrictions to assert that the exact intended `EvalError` messages are produced.

## $(date +%Y-%m-%d) - repl & export test coverage gap
**Learning:** Found testing gaps for early returns resulting from limits (e.g. cycle count 0) and terminal interaction helper loops mapping reader lines to the session state machine.
**Action:** When working on CLI interaction boundaries, try mocking `BufRead` with `Cursor` structures to unit-test logic branches without doing extensive full-system integration tests.

## $(date +%Y-%m-%d) - Coverage Gap in `eval.rs` Explicit-Time Parsing
**Learning:** `stream(...)`, `seq_sections(...)`, and explicit-time boundary errors (like mixing pattern types in `seq_sections`, exceeding 1024 cycles, or passing zero bounds) completely lacked test coverage.
**Action:** Targeted `eval_module` string compilation tests mapped to those missing lines successfully captured exact failure output strings, raising coverage by several percentage points and ensuring these edge cases cannot regress silently.

## 2024-03-24 - [Coverage Gap in `value.rs` query_unit and `eval.rs` find_call_expr_site_salt]
**Learning:** `query_unit` gracefully degrades when a pattern errors, returning an empty vector to prevent crashing the UI, but this behavior lacked a test. Furthermore, `find_call_expr_site_salt` in `eval.rs` had an unwrap that could panic without test coverage.
**Action:** Wrote tests using invalid inputs (like `slow(0)` or missing random calls) to verify the expected graceful degradation and panic behaviors respectively.
