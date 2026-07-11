## YYYY-MM-DD - Colored Comfy-Table Stats Output
**Learning:** `comfy-table` supports styling its columns directly via `Cell::new(text).fg(...)`. Trying to use `crossterm::style::Stylize` strings (e.g. `text.red()`) inside a `comfy-table` cell breaks the table's width calculations and causes alignment issues. Also, trying to remove tabular output formatting entirely in favor of a raw string log violates the goal of a dashboard-like UI.
**Action:** When adding color to `comfy-table`, use its native styling (`comfy_table::Color`). When testing these outputs in `ratatui` UI or console assertions, rely on `contains()` for unstyled substrings or use a regex to strip ANSI escape codes before asserting `eq()`. Keep the dashboard structured.

## YYYY-MM-DD - Formatting Output in orpheus-lang
**Learning:** `orpheus-lang` is the core CLI/TUI layer of the workspace. Using presentation logic like ANSI escape codes and `comfy-table` inside this crate is correct and necessary to format the text walls outputted by commands like `:explain`.
**Action:** When acting as Mosaic, continue leveraging `comfy-table` and `crossterm` inside `orpheus-lang` to provide structured data formatting and visual hierarchy, ensuring the REPL/TUI behaves like a proper dashboard instead of outputting raw text dumps.
## 2026-07-11 - Polish CLI Output and Audio Warning
**Before:** The `Audio Output Disabled` warning dumped ANSI-styled strings into the TUI, causing raw escape sequences to be rendered as text. REPL outputs combined icon and message coloring, and multi-line errors were entirely bold/red, reducing readability.
**After:** ANSI formatting is applied separately to icons and text in the CLI. Multi-line errors and warnings now highlight the first line (title) and mute the subsequent lines (details) for better visual hierarchy.
**Visuals:** Clean, dashboard-like text coloring in the CLI, properly formatted warnings in the TUI, and no raw `\u{1b}[...m` artifacts on screen.
