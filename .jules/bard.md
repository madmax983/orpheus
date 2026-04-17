## 2024-05-18 - [Missing Pedal Graph API Documentation]
**Confusion:** The virtual analog pedal API across `orpheus-dsp` and `orpheus-lang` was missing documentation for its complex types like `PedalNode`, `PedalGraphProgram`, and `PedalInstance`, leading to `missing_docs` compiler warnings and a poor developer experience.
**Clarification:** Added detailed `///` documentation blocks explaining the purpose of each DSP stage, graph structure, and stateful runtime instance. Also included executable `## Examples` doctests for constructing `PedalNode`, building a `PedalGraphProgram`, and instantiating a `PedalInstance` for audio processing. Hid boilerplate properties using `#[doc(hidden)]` to comply with Bard's philosophy.

## 2026-04-17 - [Fixed Trivial Documentation Bug]
**Confusion:** Encountered missing_docs compiler warnings on test helper functions within `crates/orpheus-lang/src/session.rs` such as `last_loaded_pattern_name` that aren't intended to be publicly documented as part of the public API surface.
**Clarification:** Hid test-specific methods from rustdoc using `#[doc(hidden)]`, ensuring the crate complies with strict `missing_docs` limits without writing unnecessary public documentation for internal APIs. Fixed an uninlined format string while at it in a havoc test.
