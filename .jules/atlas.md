**[Standardize error types]
**Tangle:** Manual implementation of `std::error::Error` for various error types like `EvalError`, `ParseError`, `TypeError`, `LoadError`, `RenderError` and `PitchLiteralError` in `orpheus-lang`, and `PatternError` in `orpheus-pattern`. This caused boilerplate and inconsistencies.
**Blueprint:** Standardized error types across the workspace by adopting the `thiserror` crate in `orpheus-lang` and `orpheus-pattern`, matching the convention already established in `orpheus-dsp`. This simplifies error definitions using `#[derive(Error)]` and `#[error(...)]` attributes.`
**[Enforce Private TUI Internal Modules]
**Tangle:** The `state` and `style` modules in `orpheus-lang/src/tui/mod.rs` were declared as `pub mod`, leaking the internal TUI implementation details.
**Blueprint:** Removed the `pub` visibility modifier from these modules in `tui/mod.rs`, converting them to `mod state;` and `mod style;`. This enforces strong module boundaries and prevents leaky abstractions.
**[Fix Leaky Abstraction in TUI Modules]
**Tangle:** The `state` and `style` modules in `crates/orpheus-lang/src/tui/mod.rs` were public (`pub mod`), leaking internal UI implementation details and breaking strict module boundaries.
**Blueprint:** Changed visibility of `state` and `style` to private (`mod`) to enforce encapsulation and prevent the "Leaky Abstraction" pattern, matching the architectural goal of maintaining a clean public API contract.
