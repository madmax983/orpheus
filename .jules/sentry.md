## 2024-05-30 - Fix Reverb Suboptimal Flops and Explicit String Cloning
**Learning:** Found clippy warnings about implicit clone from `to_string()` in `tracker.rs`, and suboptimal flops (missing mul_add usage) in `effects/reverb.rs`. Fixed these warnings and discovered test failures in `reverb.rs`. The `mul_add` substitution is not 1-to-1 equivalence unless terms are grouped properly in floating-point ops: `a * b + c * d` vs `a.mul_add(b, c * d)`.
**Action:** Always be careful around floating point optimization refactors and verify existing DSP unit tests pass afterwards.

## 2024-05-30 - Add EvalError From Tests
**Learning:** Evaluated code coverage and noticed missing tests for standard `From` conversions in `EvalError`. Added explicit unit tests to ensure different underlying error types (e.g. `TryFromIntError`, `ParseIntError`, `std::io::Error`, `std::fmt::Error`, `PatternError`) correctly map to `EvalError` string representations.
**Action:** Identify untested `From` or error mapping paths and add basic roundtrip tests to make sure error display logic isn't silently broken.

## 2024-06-25 - Improve TUI State test coverage
**Learning:** Found significant coverage gaps in the `tui::state` module tracking cursor movements, user interactions, and history state (was ~51%). Adding targeted unit tests increased coverage to ~78%. Helper functions for moving forward and backward over character boundaries and words in Unicode strings can be tested purely with mock terminal interactions on `SharedState`.
**Action:** Keep targeting frontend application states like TUI when writing tests as they often contain easily isolated logical pure state updates that developers forget to verify manually.

## 2025-10-24 - Tracker Zero Duration Formatting
**Learning:** Found a missing coverage gap where `tracker.rs` has specific logic to format and gracefully handle samples when `start_step == end_step` or when the time resolution collapses a duration down to 0 steps. Triggering these formatting paths from source evaluation requires very specific parser logic which is unstable, so explicit construction or triggering error edges using scientific notation was needed instead.
**Action:** The logic for time events that resolve to zero duration or overflow parsing requires coverage.
