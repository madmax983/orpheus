## 2024-05-30 - Fix Reverb Suboptimal Flops and Explicit String Cloning
**Learning:** Found clippy warnings about implicit clone from `to_string()` in `tracker.rs`, and suboptimal flops (missing mul_add usage) in `effects/reverb.rs`. Fixed these warnings and discovered test failures in `reverb.rs`. The `mul_add` substitution is not 1-to-1 equivalence unless terms are grouped properly in floating-point ops: `a * b + c * d` vs `a.mul_add(b, c * d)`.
**Action:** Always be careful around floating point optimization refactors and verify existing DSP unit tests pass afterwards.

## 2024-05-30 - Add EvalError From Tests
**Learning:** Evaluated code coverage and noticed missing tests for standard `From` conversions in `EvalError`. Added explicit unit tests to ensure different underlying error types (e.g. `TryFromIntError`, `ParseIntError`, `std::io::Error`, `std::fmt::Error`, `PatternError`) correctly map to `EvalError` string representations.
**Action:** Identify untested `From` or error mapping paths and add basic roundtrip tests to make sure error display logic isn't silently broken.
## 2024-06-25 - Improve TUI State test coverage
**Learning:** Found significant coverage gaps in the `tui::state` module tracking cursor movements, user interactions, and history state (was ~51%). Adding targeted unit tests increased coverage to ~78%. Helper functions for moving forward and backward over character boundaries and words in Unicode strings can be tested purely with mock terminal interactions on `SharedState`.
**Action:** Keep targeting frontend application states like TUI when writing tests as they often contain easily isolated logical pure state updates that developers forget to verify manually.
## 2024-05-30 - Add Missing From Error Conversions in EvalError
**Learning:** Found coverage gaps where `From` conversions for parser errors like `TypeError` and `LoadError` into `EvalError` were completely missing or untested, causing potential runtime/evaluation path issues when resolving files or strict typechecking.
**Action:** When adding diagnostic/parsing error types (like `TypeError`, `LoadError`), explicitly implement their `From` conversion into the central domain error enum (e.g. `EvalError`) and add simple tests for `.to_string()` propagation.

## 2024-05-30 - Fix broken rustdoc test on PedalValue::new
**Learning:** A public rustdoc test for `PedalValue::new` in `crates/orpheus-lang/src/pedal.rs` failed due to private path usage (`use orpheus_lang::pedal::...`). The path was updated to correct public re-exports (e.g. `orpheus_lang::{PedalGraph, ValidatedPedalPlan}`) and explicit cross-crate imports (e.g. `use orpheus_dsp::pedal::program::PedalNode`). Furthermore, found missing backticks inside documentation of `orpheus-dsp/src/pedal/program.rs` and fixed them based on clippy warnings.
**Action:** Verify that any rustdoc tests execute properly with `cargo test --doc`. Ensure doc-comments use fully-qualified, publicly visible imports instead of private crate-internal paths. Ensure markdown doc code blocks contain proper backticks.
