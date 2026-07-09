## YYYY-MM-DD - Colored Comfy-Table Stats Output
**Learning:** `comfy-table` supports styling its columns directly via `Cell::new(text).fg(...)`. Trying to use `crossterm::style::Stylize` strings (e.g. `text.red()`) inside a `comfy-table` cell breaks the table's width calculations and causes alignment issues. Also, trying to remove tabular output formatting entirely in favor of a raw string log violates the goal of a dashboard-like UI.
**Action:** When adding color to `comfy-table`, use its native styling (`comfy_table::Color`). When testing these outputs in `ratatui` UI or console assertions, rely on `contains()` for unstyled substrings or use a regex to strip ANSI escape codes before asserting `eq()`. Keep the dashboard structured.

## YYYY-MM-DD - Formatting Output in orpheus-lang
**Learning:** `orpheus-lang` is the core CLI/TUI layer of the workspace. Using presentation logic like ANSI escape codes and `comfy-table` inside this crate is correct and necessary to format the text walls outputted by commands like `:explain`.
**Action:** When acting as Mosaic, continue leveraging `comfy-table` and `crossterm` inside `orpheus-lang` to provide structured data formatting and visual hierarchy, ensuring the REPL/TUI behaves like a proper dashboard instead of outputting raw text dumps.

## YYYY-MM-DD - Colorizing REPL Output Prefixes
**Learning:** Applying ANSI colors to the entire message string in CLI/REPL output hides the visual hierarchy and makes the actual error or info text harder to read. Also, using string interpolation for colors can be tricky with string ownership.
**Action:** Always apply ANSI colors and styles (like `.red().bold()`) only to semantic icons (e.g., `✓`, `✗`) or prefixes (e.g., `[Warn]`) using `format_args!`, leaving the actual message text unstyled to maintain readability and hierarchy.
