## 2024-05-20 - [Session Split]
**Tangle:** The `repl.rs` file mixed IO logic (`run_stdio`, string formatting, colors) with core application state (`ReplSession`, `TransportView`, `PatternDisplayState`).
**Blueprint:** Extracted core session application state to `session.rs` to maintain a strict architectural boundary, keeping the UI/IO logic in `repl.rs` and `tui.rs` decoupled from core execution state.

## 2024-03-18 - Extracted Pattern Exporters
**Tangle:** The `crates/orpheus-lang/src/eval.rs` module had grown over 1400 lines and started turning into a "Blob", violating the Single Responsibility Principle by mixing AST evaluation with CSV exporting and audio rendering logic.
**Blueprint:** Extracted exporting functions (`export_sample_pattern_to_csv`, `export_number_pattern_to_csv`) and audio rendering functions (`render_sample_pattern_to_file`, `render_sample_pattern_to_wav`) along with `RenderError` into a dedicated `crates/orpheus-lang/src/export.rs` module.
