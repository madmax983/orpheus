**[Standardize error types]
**Tangle:** Manual implementation of `std::error::Error` for various error types like `EvalError`, `ParseError`, `TypeError`, `LoadError`, `RenderError` and `PitchLiteralError` in `orpheus-lang`, and `PatternError` in `orpheus-pattern`. This caused boilerplate and inconsistencies.
**Blueprint:** Standardized error types across the workspace by adopting the `thiserror` crate in `orpheus-lang` and `orpheus-pattern`, matching the convention already established in `orpheus-dsp`. This simplifies error definitions using `#[derive(Error)]` and `#[error(...)]` attributes.`

**[Enforce private UI module boundaries]
**Tangle:** The `tui::state` and `tui::style` modules in `orpheus-lang` were publicly exposed (`pub mod`), which could lead to leaky abstractions and weak module boundaries by allowing external crates to depend on internal UI implementation details.
**Blueprint:** Changed module visibility from `pub mod` to `mod` for `state` and `style` inside `crates/orpheus-lang/src/tui/mod.rs` to enforce strong encapsulation and hide internal TUI details from the public API.
