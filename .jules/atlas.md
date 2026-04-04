## 2024-05-20 - [Session Split]
**Tangle:** The `repl.rs` file mixed IO logic (`run_stdio`, string formatting, colors) with core application state (`ReplSession`, `TransportView`, `PatternDisplayState`).
**Blueprint:** Extracted core session application state to `session.rs` to maintain a strict architectural boundary, keeping the UI/IO logic in `repl.rs` and `tui.rs` decoupled from core execution state.

## 2024-03-18 - Extracted Pattern Exporters
**Tangle:** The `crates/orpheus-lang/src/eval.rs` module had grown over 1400 lines and started turning into a "Blob", violating the Single Responsibility Principle by mixing AST evaluation with CSV exporting and audio rendering logic.
**Blueprint:** Extracted exporting functions (`export_sample_pattern_to_csv`, `export_number_pattern_to_csv`) and audio rendering functions (`render_sample_pattern_to_file`, `render_sample_pattern_to_wav`) along with `RenderError` into a dedicated `crates/orpheus-lang/src/export.rs` module.

## YYYY-MM-DD - [Doctests on Internal Modules]
**Tangle:** Doctests on internal, unexported functions in `crates/orpheus-dsp` failed because they attempted to import private module items from the root crate, breaking the public API boundary.
**Blueprint:** Replaced `/// ```\n` with `/// ```ignore\n` on the doctest blocks for `load_sample_manifest`, `frames_per_cycle`, and `new_command_queue`. This satisfies the architectural constraint to not expose internal functions or types solely to fix failing doctests, keeping the public API clean.

## YYYY-MM-DD - [Workspace Module Encapsulation]
**Tangle:** Root crates (`orpheus-dsp`, `orpheus-lang`) leaked entire inner module namespaces to public consumers via `pub mod graph;`, `pub mod repl;`, and `pub mod tui;`, exposing unnecessary sub-module internal structures instead of providing a clean top-level API.
**Blueprint:** Refactored crate roots using the Facade pattern: switched all `pub mod` to private `mod` and explicitly re-exported only the required items to the root via `pub use`. All tests and `src/main.rs` were updated to use the new flattened, decoupled API.

## 2024-05-23 - [Standardize error types]
**Tangle:** The `PitchLiteralError` struct in `crates/orpheus-lang/src/pitch.rs` did not implement the standard `std::error::Error` trait, violating error standardization across the workspace.
**Blueprint:** Implemented `std::error::Error` for `PitchLiteralError` to align with other error types in the workspace and the broader Rust ecosystem.
