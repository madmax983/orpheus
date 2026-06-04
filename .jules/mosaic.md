## YYYY-MM-DD - Colored Comfy-Table Stats Output
**Learning:** `comfy-table` supports styling its columns directly via `Cell::new(text).fg(...)`. Trying to use `crossterm::style::Stylize` strings (e.g. `text.red()`) inside a `comfy-table` cell breaks the table's width calculations and causes alignment issues. Also, trying to remove tabular output formatting entirely in favor of a raw string log violates the goal of a dashboard-like UI.
**Action:** When adding color to `comfy-table`, use its native styling (`comfy_table::Color`). When testing these outputs in `ratatui` UI or console assertions, rely on `contains()` for unstyled substrings or use a regex to strip ANSI escape codes before asserting `eq()`. Keep the dashboard structured.

## YYYY-MM-DD - Formatting Output in orpheus-lang
**Learning:** `orpheus-lang` is the core CLI/TUI layer of the workspace. Using presentation logic like ANSI escape codes and `comfy-table` inside this crate is correct and necessary to format the text walls outputted by commands like `:explain`.
**Action:** When acting as Mosaic, continue leveraging `comfy-table` and `crossterm` inside `orpheus-lang` to provide structured data formatting and visual hierarchy, ensuring the REPL/TUI behaves like a proper dashboard instead of outputting raw text dumps.

**[Semantic Output Structuring for TUI Transcripts]**
**Learning:** Hardcoding string prefixes like `> ` or `\u{2717}` in internal state structures and relying on string matching (e.g., `starts_with`) in presentation layers makes the codebase fragile, tightly couples state and view, and complicates visual styling updates in UI toolkits like `ratatui`.
**Action:** Replace arbitrary string prefix checks with strongly typed enums (e.g., `TranscriptEntry::Input`, `TranscriptEntry::Error`) to track output state. Keep the semantic meaning decoupled from the rendering layer and apply presentation details (icons and colors) only at the moment of UI construction.
