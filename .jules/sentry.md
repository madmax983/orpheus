## 2024-05-30 - Fix Reverb Suboptimal Flops and Explicit String Cloning
**Learning:** Found clippy warnings about implicit clone from `to_string()` in `tracker.rs`, and suboptimal flops (missing mul_add usage) in `effects/reverb.rs`. Fixed these warnings and discovered test failures in `reverb.rs`. The `mul_add` substitution is not 1-to-1 equivalence unless terms are grouped properly in floating-point ops: `a * b + c * d` vs `a.mul_add(b, c * d)`.
**Action:** Always be careful around floating point optimization refactors and verify existing DSP unit tests pass afterwards.

## 2024-05-30 - Add EvalError From Tests
**Learning:** Evaluated code coverage and noticed missing tests for standard `From` conversions in `EvalError`. Added explicit unit tests to ensure different underlying error types (e.g. `TryFromIntError`, `ParseIntError`, `std::io::Error`, `std::fmt::Error`, `PatternError`) correctly map to `EvalError` string representations.
**Action:** Identify untested `From` or error mapping paths and add basic roundtrip tests to make sure error display logic isn't silently broken.
## 2024-06-25 - Improve TUI State test coverage
**Learning:** Found significant coverage gaps in the `tui::state` module tracking cursor movements, user interactions, and history state (was ~51%). Adding targeted unit tests increased coverage to ~78%. Helper functions for moving forward and backward over character boundaries and words in Unicode strings can be tested purely with mock terminal interactions on `SharedState`.
**Action:** Keep targeting frontend application states like TUI when writing tests as they often contain easily isolated logical pure state updates that developers forget to verify manually.
## 2026-04-16 - [Testing TUI style pure functions without full engine]
**Learning:** Testing pure TUI formatting functions (like `transport_status_line`) can often be accomplished efficiently by constructing a real `ReplSession` with a stubbed `EngineHandle`, and simulating state transitions via `eval_line()` rather than directly mocking the underlying state structs.
**Action:** Use `ReplSession::with_engine(EngineHandle::stub())` and `session.eval_line()` to generate complex UI state views (like `TransportView`, `MixerView`) for unit testing TUI layout elements without needing complex mocks.
## 2026-04-16 - Add format_cycle_position test
**Learning:** Found format_cycle_position in tui/style.rs was completely untested.
**Action:** Add unit test for formatting the cycle position.
