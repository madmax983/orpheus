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
**[Fix Leaky Abstraction in Value and FunctionValue Enums]
**Tangle:** The `Value` and `FunctionValue` enums in `orpheus-lang` were public and exposed inner payload types like `ArpDirectionValue`, `PitchClassSetValue`, `BuiltinFn`, `BuiltinKind`, and `UserFn` as part of their variants. However, these inner types were not re-exported in the crate's `lib.rs`, creating a leaky abstraction where consumers could match on the variants but could not explicitly name the types of the values they extracted.
**Blueprint:** Explicitly re-exported `ArpDirectionValue`, `BuiltinFn`, `BuiltinKind`, `PitchClassSetValue`, and `UserFn` from the `value` module inside `crates/orpheus-lang/src/lib.rs` to ensure all publicly reachable types are fully nameable.
**[Enforce Private SuperCollider Export Module]
**Tangle:** The  module in  was declared as , leaking the internal implementation details of the SuperCollider export module.
**Blueprint:** Changed  to  in . This enforces strong module boundaries by keeping the module internal while the intended public APIs ( and ) are explicitly exposed via .
**[Enforce Private SuperCollider Export Module]
**Tangle:** The `supercollider_export` module in `orpheus-lang/src/lib.rs` was declared as `pub mod`, leaking the internal implementation details of the SuperCollider export module.
**Blueprint:** Changed `pub mod supercollider_export;` to `pub(crate) mod supercollider_export;` in `crates/orpheus-lang/src/lib.rs`. This enforces strong module boundaries by keeping the module internal while the intended public APIs (`export_number_pattern_to_supercollider` and `export_sample_pattern_to_supercollider`) are explicitly exposed via `pub use`.
**[Fix Leaky Abstraction in ValidatedPedalPlan and GatePatternValue Enums]
**Tangle:** The `ValidatedPedalPlan` and `Value` enums in `orpheus-lang` were public and exposed inner payload types like `ValidatedPedalBinding`, `ValidatedPedalNode`, `PedalNodeKind`, and `GatePatternValue` as part of their variants. However, these inner types were either private or not re-exported in the crate's `lib.rs`, creating a leaky abstraction where consumers could not explicitly name the types of the values they extracted or the compiler complained about privacy.
**Blueprint:** Explicitly re-exported `GatePatternValue`, `ValidatedPedalBinding`, and `ValidatedPedalNode` from the `value` and `pedal` modules inside `crates/orpheus-lang/src/lib.rs` and made `PedalNodeKind` public to ensure all publicly reachable types are fully nameable and privacy boundaries are respected.
**[Fix Leaky Abstraction in Type Enum]
**Tangle:** The `Type` enum in `orpheus-lang` was public and exposed the inner payload type `TypeVarId` as part of its `Var` variant. However, this inner type was not re-exported in the crate's `lib.rs`, creating a leaky abstraction.
**Blueprint:** Explicitly re-exported `TypeVarId` from the `types` module inside `crates/orpheus-lang/src/lib.rs` to ensure all publicly reachable types are fully nameable.

**[Float Epsilon Equality Bug]**
**Learning:** Checking floating-point equality in tests using `val - expected < f64::EPSILON` is a logical bug because a negative difference will always evaluate as less than epsilon, causing false positives.
**Action:** Always apply `.abs()` to the difference before comparing to epsilon: `(val - expected).abs() < f64::EPSILON`.

**[Simplify IO Other Error]**
**Learning:** Instantiating generic IO errors using `std::io::Error::new(std::io::ErrorKind::Other, "message")` triggers `clippy::io_other_error`.
**Action:** Use the cleaner, modern shorthand `std::io::Error::other("message")`.
**[Fix Leaky Abstractions and Broken Doctests]
**Tangle:** Several `orpheus_lang` public APIs referenced internal, private types (like `GraphBinding`, `TypeEnv`, and `TypeScheme`), causing leaky abstractions. Furthermore, missing getter methods for variants like `FunctionValue` caused test failures. Several documentation tests were bypassing the crate facade by reaching directly into private submodules.
**Blueprint:** Explicitly re-exported internal types (`GraphBinding`, `TypeEnv`, `TypeScheme`) in `lib.rs` and the `types/mod.rs` module. Refactored doctests to utilize the public facade and exposed a `Value::as_function` method to fulfill the expected public API contract without exposing underlying structural data prematurely.

**[Fix Leaky Abstraction in AST GraphBinding]**
**Tangle:** The `Expr` enum in `orpheus-lang` was public and exposed the inner payload type `GraphBinding` as part of its `Graph` variant. However, this inner type was not re-exported in the crate's `lib.rs`, creating a leaky abstraction where consumers could match on the variant but could not explicitly name the type of the value they extracted.
**Blueprint:** Explicitly re-exported `GraphBinding` from the `ast` module inside `crates/orpheus-lang/src/lib.rs` to ensure all publicly reachable types are fully nameable.
**[Enforce Private Explain Module]
**Tangle:** The `explain` module in `orpheus-lang/src/lib.rs` and its internal `Explain` trait and `explain_table` function were declared as `pub`, leaking internal REPL table rendering details to the public API.
**Blueprint:** Changed the visibility of the `Explain` trait and `explain_table` function to `pub(crate)` in `crates/orpheus-lang/src/explain.rs`. Removed the `pub use explain::Explain;` re-export from `crates/orpheus-lang/src/lib.rs` and changed the module declaration to `pub(crate) mod explain;`. This strictly enforces internal encapsulation.

**[Encapsulate `types::env` Module]
**Tangle:** The `crates/orpheus-lang/src/types/mod.rs` module leaked its internal implementation details via `pub mod env;`, violating the strict `pub(crate)` encapsulation boundary and creating a leaky abstraction where external consumers could bypass the explicit `pub use` API contract.
**Blueprint:** Altered `pub mod env;` to `pub(crate) mod env;` to strict structural boundaries. The public API remains functionally unchanged because the necessary types (`TypeEnv`, `TypeScheme`) are properly exported via `pub use env::*`.
