**[Standardize error types]
**Tangle:** Manual implementation of `std::error::Error` for various error types like `EvalError`, `ParseError`, `TypeError`, `LoadError`, `RenderError` and `PitchLiteralError` in `orpheus-lang`, and `PatternError` in `orpheus-pattern`. This caused boilerplate and inconsistencies.
**Blueprint:** Standardized error types across the workspace by adopting the `thiserror` crate in `orpheus-lang` and `orpheus-pattern`, matching the convention already established in `orpheus-dsp`. This simplifies error definitions using `#[derive(Error)]` and `#[error(...)]` attributes.`
**[Enforce Private Module Boundaries in `tui`]**
**Tangle:** The `tui` module in `orpheus-lang` was publicly exporting internal implementation modules `state` and `style` (`pub mod state;`, `pub mod style;`). This violated the architectural boundary rule by leaking private UI state and styling abstractions into the broader crate namespace, increasing the risk of external coupling to internal UI components.
**Blueprint:** Refactored the `tui` module to restrict the visibility of its submodules to private (`mod state;`, `mod style;`). This ensures strong encapsulation of the TUI implementation details, maintaining a clean public API contract for the `orpheus-lang` crate and preventing the "Leaky Abstraction" anti-pattern.
**[Enforce Private TUI Internal Modules]
**Tangle:** The `state` and `style` modules in `orpheus-lang/src/tui/mod.rs` were declared as `pub mod`, leaking the internal TUI implementation details.
**Blueprint:** Removed the `pub` visibility modifier from these modules in `tui/mod.rs`, converting them to `mod state;` and `mod style;`. This enforces strong module boundaries and prevents leaky abstractions.
