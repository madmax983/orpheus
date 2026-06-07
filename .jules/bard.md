## 2024-05-19 - [Missing documentation on public functions and structs]
**Confusion:** The crate had multiple undocumented functions and struct fields which resulted in clippy warnings for missing documentation.
**Clarification:** Documented missing functions in `builtins.rs`, `loader.rs`, `pedal.rs`, `env.rs`, and `value.rs`, providing clear doc tests and clarifying their roles within the AST and evaluation engine. Also removed unneeded `#[must_use]` attributes that caused clippy warnings.
