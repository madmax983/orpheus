## YYYY-MM-DD - Colored Comfy-Table Stats Output
**Learning:** `comfy-table` supports styling its columns directly via `Cell::new(text).fg(...)`. Trying to use `crossterm::style::Stylize` strings (e.g. `text.red()`) inside a `comfy-table` cell breaks the table's width calculations and causes alignment issues. Also, trying to remove tabular output formatting entirely in favor of a raw string log violates the goal of a dashboard-like UI.
**Action:** When adding color to `comfy-table`, use its native styling (`comfy_table::Color`). When testing these outputs in `ratatui` UI or console assertions, rely on `contains()` for unstyled substrings or use a regex to strip ANSI escape codes before asserting `eq()`. Keep the dashboard structured.

## YYYY-MM-DD - Formatting Output in orpheus-lang
**Learning:** `orpheus-lang` is the core CLI/TUI layer of the workspace. Using presentation logic like ANSI escape codes and `comfy-table` inside this crate is correct and necessary to format the text walls outputted by commands like `:explain`.
**Action:** When acting as Mosaic, continue leveraging `comfy-table` and `crossterm` inside `orpheus-lang` to provide structured data formatting and visual hierarchy, ensuring the REPL/TUI behaves like a proper dashboard instead of outputting raw text dumps.
## 2024-07-12 - Mosaic UI Polish: Dashboard Tables
**Before:** The REPL commands output text walls or basic tables with `UTF8_BORDERS_ONLY` and white/yellow text, which lacked visual hierarchy and looked like log files.
**After:** All REPL table outputs (`:env`, `:help`, `:mixer`, `:explain`, `:stats`, `:roll`) now use a polished `UTF8_FULL` preset with `UTF8_ROUND_CORNERS` and `UTF8_SOLID_INNER_BORDERS`. Table headers pop out with Cyan text.
**Visuals:** Tables now look like structured dashboards. The Z-Pattern scanning layout is clearer because the colored headers easily differentiate rows from columns.
