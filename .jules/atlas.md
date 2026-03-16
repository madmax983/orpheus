
## 2026-03-14 - Extract Session state from REPL IO
**Tangle:** The Sprawl - `ReplSession`, which manages the core state of the application (audio engine interaction, sample bank, bindings, and transport), was deeply intertwined with the IO logic in `repl.rs`. Furthermore, `tui.rs` was forced to reach into `repl.rs` to grab the session logic.
**Blueprint:** Extracted `ReplSession` (renamed to `Session`), `TransportView`, and `PatternDisplayState` into a dedicated `session.rs` module. This enforces a clear separation of concerns, allowing both `repl.rs` and `tui.rs` to act strictly as UI layers consuming the shared session state.
