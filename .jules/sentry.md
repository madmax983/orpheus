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

## 2024-06-25 - Add Error From Tests
**Learning:** Evaluated code coverage and noticed missing tests for `From` conversions to `EvalError` for `TypeError` and `LoadError`. Also missing tests for `From` conversions to `TypeError` for `ParseError`.
**Action:** Identify untested `From` or error mapping paths and add basic roundtrip tests to make sure error display logic isn't silently broken.

## 2024-05-18 - [Unwrap Verification]
**Learning:** Tools like `grep` can match `unwrap_or` and `unwrap_or_else` when looking for `unwrap`. Furthermore, when calculating capacity based on an iterator's `size_hint` where the items are references to structures stored in memory, `upper` bounds are bounded by physical memory limitations, meaning operations like `upper * 2` are very unlikely to overflow `usize` in standard 64-bit systems.
**Action:** Use precise regex (like `\.unwrap\(\)`) to search for panics. Avoid modifying safe capacity logic just because `usize::MAX` theoretically overflows it if the value can never approach `usize::MAX` in reality.

## 2026-04-16 - Export zero-cycle count tests
**Learning:** Evaluated export handlers for HTML, Markdown, CSV, Tracker, text, and other formatters. While logic safely catches zero `cycle_count` conditions with `EvalError` across the `export.rs` functions, dedicated unit tests verifying this error outcome were only added to some files and entirely missing in `html.rs`, `mermaid.rs`, `osu_export.rs`, `scad_export.rs`, `sonic_pi_export.rs`, and `svg.rs`.
**Action:** Ensure boundary assertions and error branches in common data extraction patterns (like exporting media patterns) have corresponding regression tests written across all format implementations, rather than relying on one format's tests to cover the identical logic structure everywhere.
## 2024-06-25 - Export zero-cycle count tests
**Learning:** Evaluated export handlers for HTML, Markdown, CSV, Tracker, text, and other formatters. While logic safely catches zero `cycle_count` conditions with `EvalError` across the `export.rs` functions, dedicated unit tests verifying this error outcome were only added to some files and entirely missing in `ascii_roll.rs`, `number_roll.rs`, `midi_export.rs`, and `srt.rs`.
**Action:** Ensure boundary assertions and error branches in common data extraction patterns (like exporting media patterns) have corresponding regression tests written across all format implementations, rather than relying on one format's tests to cover the identical logic structure everywhere.
## 2025-05-02 - Eliminate unwrap() using stable Rust constructs
**Learning:** Replacing `.unwrap()` with `if let Some` and `&&` (let chains) is an unstable Rust feature. Using it causes the compiler to reject the build.
**Action:** Always use either nested `if` statements with `#[allow(clippy::collapsible_if)]` or modern iterator methods like `.is_some_and(...)` when safely unpacking values conditionally on stable Rust.

## 2024-10-27 - std::io::Error::other Shorthand
**Learning:** Found clippy warning `clippy::io_other_error` in `test_havoc_error.rs` when using `std::io::Error::new(std::io::ErrorKind::Other, "...")`.
**Action:** Use the shorthand `std::io::Error::other("...")` when constructing generic I/O errors to satisfy clippy and improve readability.

## 2024-10-27 - float_equality_without_abs
**Learning:** Found clippy warning `clippy::float_equality_without_abs` in `eval.rs` when checking float equality without `abs()`. `val - 42.0 < f64::EPSILON` is unsafe because a very negative number is also less than epsilon.
**Action:** Always use `.abs()` when comparing floats to epsilon: `(val - expected).abs() < f64::EPSILON`.
## 2025-02-27 - [Re-exporting Private Sub-Modules for Doctests]
**Learning:** When using `#[doc(hidden)] pub use parent::child::Type;` at the crate root to mock internal functions for doctests, the compiler will throw `E0603: module is private` if the intermediate `child` module is private within its `parent`.
**Action:** Ensure the intermediate module is marked as `pub mod` or `pub(crate) mod` inside its parent (e.g., `pub mod env;` in `types/mod.rs`) before re-exporting its contents at the crate root.
## 2024-05-30 - Fix non-exhaustive matches for Hex and Bin in value.rs
**Learning:** Found non-exhaustive pattern match errors in `crates/orpheus-lang/src/value.rs` around the newly added `Hex` and `Bin` BuiltinKinds when running `cargo test --all-targets --all-features`.
**Action:** The solution was to find exhaustive `match` statements across the repository that use `BuiltinKind` and add matches for `BuiltinKind::Hex` and `BuiltinKind::Bin`. Also added missing arguments test cases for `hex` and `bin` to value.rs.

## 2024-06-25 - Improve coverage in ast.rs
**Learning:** Found coverage gaps in `crates/orpheus-lang/src/ast.rs` for `references_ident` across several AST traversal nodes (e.g., `Pipe`, `Binary`, `Call`). Added tests to ensure all `Expr` enum variants correctly traverse when checking for identifier references.
**Action:** When adding or refactoring recursive AST methods, ensure comprehensive unit tests cover traversal logic for all node variants to avoid shadowing bugs or false negatives.
