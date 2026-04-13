## 2024-05-30 - Fix Reverb Suboptimal Flops and Explicit String Cloning
**Learning:** Found clippy warnings about implicit clone from `to_string()` in `tracker.rs`, and suboptimal flops (missing mul_add usage) in `effects/reverb.rs`. Fixed these warnings and discovered test failures in `reverb.rs`. The `mul_add` substitution is not 1-to-1 equivalence unless terms are grouped properly in floating-point ops: `a * b + c * d` vs `a.mul_add(b, c * d)`.
**Action:** Always be careful around floating point optimization refactors and verify existing DSP unit tests pass afterwards.

## $(date +%Y-%m-%d) - repl & export test coverage gap
**Learning:** Found testing gaps for early returns resulting from limits (e.g. cycle count 0) and terminal interaction helper loops mapping reader lines to the session state machine.
**Action:** When working on CLI interaction boundaries, try mocking `BufRead` with `Cursor` structures to unit-test logic branches without doing extensive full-system integration tests.

## $(date +%Y-%m-%d) - Coverage Gap in `eval.rs` Explicit-Time Parsing
**Learning:** `stream(...)`, `seq_sections(...)`, and explicit-time boundary errors (like mixing pattern types in `seq_sections`, exceeding 1024 cycles, or passing zero bounds) completely lacked test coverage.
**Action:** Targeted `eval_module` string compilation tests mapped to those missing lines successfully captured exact failure output strings, raising coverage by several percentage points and ensuring these edge cases cannot regress silently.

## 2024-03-24 - [Coverage Gap in `value.rs` query_unit and `eval.rs` find_call_expr_site_salt]
**Learning:** `query_unit` gracefully degrades when a pattern errors, returning an empty vector to prevent crashing the UI, but this behavior lacked a test. Furthermore, `find_call_expr_site_salt` in `eval.rs` had an unwrap that could panic without test coverage.
**Action:** Wrote tests using invalid inputs (like `slow(0)` or missing random calls) to verify the expected graceful degradation and panic behaviors respectively.
## 2024-05-30 - Add EvalError From Tests
**Learning:** Evaluated code coverage and noticed missing tests for standard `From` conversions in `EvalError`. Added explicit unit tests to ensure different underlying error types (e.g. `TryFromIntError`, `ParseIntError`, `std::io::Error`, `std::fmt::Error`, `PatternError`) correctly map to `EvalError` string representations.
**Action:** Identify untested `From` or error mapping paths and add basic roundtrip tests to make sure error display logic isn't silently broken.
