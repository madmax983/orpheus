## 2024-05-18 - [Missing Pedal Graph API Documentation]
**Confusion:** The virtual analog pedal API across `orpheus-dsp` and `orpheus-lang` was missing documentation for its complex types like `PedalNode`, `PedalGraphProgram`, and `PedalInstance`, leading to `missing_docs` compiler warnings and a poor developer experience.
**Clarification:** Added detailed `///` documentation blocks explaining the purpose of each DSP stage, graph structure, and stateful runtime instance. Also included executable `## Examples` doctests for constructing `PedalNode`, building a `PedalGraphProgram`, and instantiating a `PedalInstance` for audio processing. Hid boilerplate properties using `#[doc(hidden)]` to comply with Bard's philosophy.
## 2024-05-19 - [Missing Doctests and Conversion Documentations]
**Confusion:** The core runtime value type `SampleEvent` lacked executable doctests, leaving users to guess the default behaviors of parameters like `gain()`, `hpf_cutoff_hz()`, and `lpf_cutoff_hz()`. Additionally, tests for converting structural error types (`LoadError`, `TypeError`) into `EvalError` were missing.
**Clarification:** Added comprehensive `///` doctests using `Value::as_sample_pattern()` querying examples to clearly demonstrate how `SampleEvent` data is extracted and modified in the engine. Wrote missing `.to_string()` unit tests in `crates/orpheus-lang/src/eval.rs` to verify that `EvalError::from` propagating works smoothly without dropping information.
## 2026-04-17 - [Missing Error Example Documentations]
**Confusion:** Several functions returning `Result` types lacked `## Examples` in their documentation, which violates the `# Examples` rule.
**Clarification:** Added executable doctests as `## Examples` for functions across `orpheus-lang` including `parse_module`, `infer_module`, `render_ascii_roll`, and `render_ascii_number_roll` to demonstrate successful execution flows and clarify return types.
## 2024-05-19 - [Missing Value Runtime Documentation]
**Confusion:** The core runtime pattern evaluation methods (`query_unit`, `try_query`, and `try_query_unit`) on `SamplePatternValue` and `NumberPatternValue` in `value.rs` lacked executable doctests and narrative documentation explaining how the runtime evaluates these structures across time, causing users to misunderstand how patterns are materialized.
**Clarification:** Added comprehensive `///` narrative documentation explaining the delayed computation model and `## Examples` doctest blocks illustrating successful materialization of `Event` sequences across arbitrary or default unit time spans.
## 2024-05-20 - [Eliminating Getter Noise in Public APIs]
**Confusion:** Functions named like `get`, `summary`, or `phase` were documented simply with "Gets the..." or "Returns the...", which violates the Bard philosophy of explaining *why* a function exists and what it represents within the domain context. These descriptions assume the user already understands the internal architecture, creating a "getter noise" anti-pattern.
**Clarification:** Rewrote the documentation for `sample_bank.rs`, `session.rs`, and `math.rs` to frame their descriptions around their purpose (e.g., synchronizing REPL UI state, memory-resident audio buffers). Added `## Examples` executable doctests where appropriate for public functions to provide concrete usage context without relying on internal knowledge.
## 2026-04-23 - [Graph API 'Dead End' Errors and Trait Usage]
**Confusion:** The `GraphError` enum did not explain how users should recover from channel mismatch errors (a "Dead End"), and core abstractions like the `Node` trait lacked examples demonstrating how to actually implement a custom node.
**Clarification:** Added a "Recovery" section and executable doctest to `GraphError` demonstrating how mismatch errors are formed. Added an executable `Doubler` node implementation example to the `Node` trait to bridge the abstraction gap.
## 2024-05-19 - Documenting SclError variants
**Confusion:** The fields `expected` and `actual` inside the `Count` variant of `SclError` in `scl.rs` lacked documentation, leading to warnings when running `cargo doc` with strict lints.
**Clarification:** Added missing doc comments explaining what `expected` and `actual` mean in the context of Scala `.scl` file parsing.
## 2024-05-25 - [Missing Test Assertion Documentation]
**Confusion:** The `last_loaded_pattern_name` function was entirely undocumented, causing warnings when strict documentation lints were applied. It was unclear why this test-only helper existed.
**Clarification:** Added a descriptive comment explaining that it is used to expose the name of the most recently evaluated pattern for assertions, helping users understand *why* it exists in the REPL session struct.

## 2024-05-18 - [Missing DSP Effect Documentation]
**Confusion:** The stateful DSP effects like  and  lacked doc comments and executable examples, obscuring how they integrate with the audio graph and language runtime.
**Clarification:** Added narrative  documentation to the structs and their core methods (, /, , ). Included  using  to demonstrate instantiation via the public API wrapper, as the structs themselves are not exported at the crate root.
## 2024-05-18 - [Missing DSP Effect Documentation]
**Confusion:** The stateful DSP effects like `DelayState` and `ReverbState` lacked doc comments and executable examples, obscuring how they integrate with the audio graph and language runtime.
**Clarification:** Added narrative `///` documentation to the structs and their core methods (`new`, `sync_timing`/`sync_spec`, `process_frame`, `reset`). Included `## Examples` using `BusEffectState::from_spec` to demonstrate instantiation via the public API wrapper, as the structs themselves are not exported at the crate root.
## 2024-05-19 - [Missing Value Enum Variant Documentation]
**Confusion:** The massive `BuiltinKind` enum and the gate pattern enums (`GatePatternValue`, `ArpDirectionValue`) in `value.rs` had dozens of variants missing documentation, leading to `missing_docs` warnings and obfuscated API context for language users.
**Clarification:** Added detailed `///` documentation to all missing variants describing their musical function rather than just repeating their names. Included `## Examples` doctests for the complex ones like `every`, `when`, and `strum` to show how they operate within the language.

## 2024-05-19 - [Missing Module Level Test Documentation]
**Confusion:** The integration test files lacked module-level documentation `//!`, violating the Bard philosophy of explaining *why* the test suite exists and the scope of its verifications.
## 2026-06-23 - [Test Flakes Caused by Hidden OS Dependencies]
**Confusion:** A test (`vst3_descriptor_uses_standard_os_search_paths`) failed on the CI pipeline as it strictly expected exact uppercase "VST3" for system paths, but paths could include lowercased variants like "vst3" depending on the underlying OS.
**Clarification:** Changed the test to use `to_ascii_lowercase().contains("vst3")` to make it case-insensitive, resolving the hidden dependency on OS path casing.
