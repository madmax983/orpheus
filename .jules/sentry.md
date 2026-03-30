## 2024-03-24 - [Coverage Gap in `eval.rs` Pattern Evaluation]
**Learning:** The evaluator logic for checking pattern types (e.g. `eval_structural_pattern`, `unsupported_pattern_item_error`) and verifying explicit-time restrictions (like `stream(...)` vs implicit `seq(...)`) contained many robust error messages that were completely untested.
**Action:** Use `cargo llvm-cov` to identify missing error-handling branch coverage. Write targeted unit tests utilizing `eval_module` that intentionally violate structural grammar and explicit-time restrictions to assert that the exact intended `EvalError` messages are produced.

## $(date +%Y-%m-%d) - repl & export test coverage gap
**Learning:** Found testing gaps for early returns resulting from limits (e.g. cycle count 0) and terminal interaction helper loops mapping reader lines to the session state machine.
**Action:** When working on CLI interaction boundaries, try mocking `BufRead` with `Cursor` structures to unit-test logic branches without doing extensive full-system integration tests.

## $(date +%Y-%m-%d) - Coverage Gap in `eval.rs` Explicit-Time Parsing
**Learning:** `stream(...)`, `seq_sections(...)`, and explicit-time boundary errors (like mixing pattern types in `seq_sections`, exceeding 1024 cycles, or passing zero bounds) completely lacked test coverage.
**Action:** Targeted `eval_module` string compilation tests mapped to those missing lines successfully captured exact failure output strings, raising coverage by several percentage points and ensuring these edge cases cannot regress silently.
## 2024-10-24 - [NumberPatternValue::query_unit graceful degradation test]
**Learning:** Pattern structures queried via `query_unit` gracefully handle runtime evaluation errors like span arithmetic bounds checks by silently catching `try_query_unit` errors and returning an empty sequence (`Vec::new()`)
**Action:** Wrote an explicit test `number_pattern_query_unit_degrades_gracefully_on_overflow` to ensure this specific error handling behavior is continuously verified and prevents silent regressions that might cause panic points if refactored improperly.

## 2024-10-24 - [Coverage Gap in `apply_user_function` Function Invocation]
**Learning:** The evaluation pipeline code for applying arguments to user-defined functions lacked test coverage around function arity boundaries, specifically the case of successfully currying functions and the over-application error handling branch.
**Action:** Always check `eval.rs` helper methods for missing branch coverage around dynamic constraints (like function arguments length checks) and ensure those failure cases are covered via `assert_eval_error_contains`.
