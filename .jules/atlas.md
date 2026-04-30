**[Standardize error types]
**Tangle:** Manual implementation of `std::error::Error` for various error types like `EvalError`, `ParseError`, `TypeError`, `LoadError`, `RenderError` and `PitchLiteralError` in `orpheus-lang`, and `PatternError` in `orpheus-pattern`. This caused boilerplate and inconsistencies.
**Blueprint:** Standardized error types across the workspace by adopting the `thiserror` crate in `orpheus-lang` and `orpheus-pattern`, matching the convention already established in `orpheus-dsp`. This simplifies error definitions using `#[derive(Error)]` and `#[error(...)]` attributes.`
**[Enforce Private TUI Internal Modules]
**Tangle:** The `state` and `style` modules in `orpheus-lang/src/tui/mod.rs` were declared as `pub mod`, leaking the internal TUI implementation details.
**Blueprint:** Removed the `pub` visibility modifier from these modules in `tui/mod.rs`, converting them to `mod state;` and `mod style;`. This enforces strong module boundaries and prevents leaky abstractions.

**[Enforce Private Mermaid Module]
**Tangle:** The `mermaid` module in `orpheus-lang/src/lib.rs` was declared as `pub mod`, leaking the internal implementation details of the Mermaid export module.
**Blueprint:** Changed `pub mod mermaid;` to `pub(crate) mod mermaid;` in `crates/orpheus-lang/src/lib.rs`. This enforces strong module boundaries by keeping the module internal while the intended public API (`export_sample_pattern_to_mermaid_gantt`) is explicitly exposed via `pub use`.

**[Enforce Private Number Roll Module]
**Tangle:** The `number_roll` module in `orpheus-lang/src/lib.rs` was declared as `pub mod`, leaking the internal implementation details of the ASCII number roll module.
**Blueprint:** Changed `pub mod number_roll;` to `pub(crate) mod number_roll;` in `crates/orpheus-lang/src/lib.rs`. This enforces strong module boundaries by keeping the module internal while the intended public API (`render_ascii_number_roll`) is explicitly exposed via `pub use`.

**[Standardize EvalError type]
**Tangle:** Manual implementation of `From` for cloneable error types in `EvalError` inside `crates/orpheus-lang/src/eval.rs`, causing boilerplate and losing inner type structure.
**Blueprint:** Converted `EvalError` from a flat struct to an enum using the `thiserror` crate's `#[from]` attribute for cloneable types, standardizing error boundaries.

**[Enforce Private Type Inference Environment]
**Tangle:** The `TypeEnv` and `TypeScheme` structs in `orpheus-lang/src/types/env.rs` were declared as `pub struct`, unnecessarily leaking the internal type-checker abstractions to the public API where only `TypedModule` is expected to be consumed.
**Blueprint:** Modified `TypeEnv` and `TypeScheme` (and their respective methods) to use `pub(crate)` visibility instead. This reinforces strong module boundaries and correctly encapsulates the language's inference engine implementation details.
**[Enforce Public Structure inside Private Module]
**Tangle:** The `TypeScheme` and `TypeEnv` structs in `orpheus-lang/src/types/env.rs` were declared as `pub(crate) struct`, which triggers `clippy::redundant_pub_crate` because the parent module `env` is private.
**Blueprint:** Modified `TypeScheme` and `TypeEnv` to use `pub` visibility instead of `pub(crate)`. This satisfies Clippy while correctly maintaining the private boundary since the module itself is private, making the items effectively crate-visible.
