## 2024-05-18 - [Missing Pedal Graph API Documentation]
**Confusion:** The virtual analog pedal API across `orpheus-dsp` and `orpheus-lang` was missing documentation for its complex types like `PedalNode`, `PedalGraphProgram`, and `PedalInstance`, leading to `missing_docs` compiler warnings and a poor developer experience.
**Clarification:** Added detailed `///` documentation blocks explaining the purpose of each DSP stage, graph structure, and stateful runtime instance. Also included executable `## Examples` doctests for constructing `PedalNode`, building a `PedalGraphProgram`, and instantiating a `PedalInstance` for audio processing. Hid boilerplate properties using `#[doc(hidden)]` to comply with Bard's philosophy.
## 2025-01-20 - [Fixing Cargo Doc Warnings]
**Confusion:** It's unclear how to resolve  on declarative UI components like ratatui  generations without destroying readability.
**Clarification:** Add `#[allow(clippy::too_many_lines)]` to the function to prioritize clear layout structure over arbitrary line limits.

## 2025-01-20 - [Fixing Cargo Doc Warnings]
**Confusion:** It's unclear how to resolve clippy too_many_lines on declarative UI components like ratatui Vec<Line> generations without destroying readability.
**Clarification:** Add #[allow(clippy::too_many_lines)] to the function to prioritize clear layout structure over arbitrary line limits.
