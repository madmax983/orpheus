## 2024-05-30 - Fix Reverb Suboptimal Flops and Explicit String Cloning
**Learning:** Found clippy warnings about implicit clone from `to_string()` in `tracker.rs`, and suboptimal flops (missing mul_add usage) in `effects/reverb.rs`. Fixed these warnings and discovered test failures in `reverb.rs`. The `mul_add` substitution is not 1-to-1 equivalence unless terms are grouped properly in floating-point ops: `a * b + c * d` vs `a.mul_add(b, c * d)`.
**Action:** Always be careful around floating point optimization refactors and verify existing DSP unit tests pass afterwards.

## 2024-05-30 - Add EvalError From Tests
**Learning:** Evaluated code coverage and noticed missing tests for standard `From` conversions in `EvalError`. Added explicit unit tests to ensure different underlying error types (e.g. `TryFromIntError`, `ParseIntError`, `std::io::Error`, `std::fmt::Error`, `PatternError`) correctly map to `EvalError` string representations.
**Action:** Identify untested `From` or error mapping paths and add basic roundtrip tests to make sure error display logic isn't silently broken.
## 2024-06-13 - [Correctness of TUI State Navigation]
**Learning:** Found that testing TUI state logic (`SharedState`) directly using its methods is an effective way to verify cursor movements and string manipulation behavior without rendering frames or instantiating a full terminal backend. Setting up mocks for `TransportView` using `ReplSession` handles UI style testing well.
**Action:** When testing TUI or view layer components, decouple the underlying logic from terminal drawing code and test state mutations using unit tests with mock engines. Avoid writing tests based on trial-and-error values, and ensure comments strictly reflect business logic.
