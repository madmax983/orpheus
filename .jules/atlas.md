**[Standardize error types]
**Tangle:** Manual implementation of `std::error::Error` for various error types like `EvalError`, `ParseError`, `TypeError`, `LoadError`, `RenderError` and `PitchLiteralError` in `orpheus-lang`, and `PatternError` in `orpheus-pattern`. This caused boilerplate and inconsistencies.
**Blueprint:** Standardized error types across the workspace by adopting the `thiserror` crate in `orpheus-lang` and `orpheus-pattern`, matching the convention already established in `orpheus-dsp`. This simplifies error definitions using `#[derive(Error)]` and `#[error(...)]` attributes.`
**[Enforce Private TUI Internal Modules]
**Tangle:** The `state` and `style` modules in `orpheus-lang/src/tui/mod.rs` were declared as `pub mod`, leaking the internal TUI implementation details.
**Blueprint:** Removed the `pub` visibility modifier from these modules in `tui/mod.rs`, converting them to `mod state;` and `mod style;`. This enforces strong module boundaries and prevents leaky abstractions.

**[Enforce Private TUI Internal Modules]
**Tangle:** The `state` and `style` modules in `orpheus-lang/src/tui/mod.rs` were declared as `pub mod` (or their internal helpers as `pub`), leaking the internal TUI implementation details and presentation layer logic outside the module (The "Leaky Abstraction").
**Blueprint:** Encapsulated UI formatting strictly within the `orpheus-lang::tui` boundary by restricting visibility to `pub(crate)`.
