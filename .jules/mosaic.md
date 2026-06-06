## YYYY-MM-DD - Colored Comfy-Table Stats Output
**Learning:** `comfy-table` supports styling its columns directly via `Cell::new(text).fg(...)`. Trying to use `crossterm::style::Stylize` strings (e.g. `text.red()`) inside a `comfy-table` cell breaks the table's width calculations and causes alignment issues. Also, trying to remove tabular output formatting entirely in favor of a raw string log violates the goal of a dashboard-like UI.
**Action:** When adding color to `comfy-table`, use its native styling (`comfy_table::Color`). When testing these outputs in `ratatui` UI or console assertions, rely on `contains()` for unstyled substrings or use a regex to strip ANSI escape codes before asserting `eq()`. Keep the dashboard structured.

## YYYY-MM-DD - Formatting Output in orpheus-lang
**Learning:** `orpheus-lang` is the core CLI/TUI layer of the workspace. Using presentation logic like ANSI escape codes and `comfy-table` inside this crate is correct and necessary to format the text walls outputted by commands like `:explain`.
**Action:** When acting as Mosaic, continue leveraging `comfy-table` and `crossterm` inside `orpheus-lang` to provide structured data formatting and visual hierarchy, ensuring the REPL/TUI behaves like a proper dashboard instead of outputting raw text dumps.
**[Replacing String Matching in TUI Transcript with Enum]**
**Before:** The UI was heavily dependent on brittle prefix matching (e.g., `entry.starts_with("\u{2717}")`) to parse UI styling out of plain strings.
**After:** Converted `transcript: Vec<String>` to `transcript: Vec<TranscriptEntry>`. We now use a strongly typed `enum TranscriptEntry` to store `Input`, `Success`, `Error`, `Warning` and `Info` events.
**Visuals:** Visually identical output, but structurally clean! Formatting is correctly mapped into UI space only right before calling the rendering layer, making the TUI State completely detached from raw styling artifacts.
