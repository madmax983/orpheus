**[Extracted Session core from UI logic]**
**Tangle:** `crates/orpheus-lang/src/repl.rs` acted as a "God Struct" housing the core `Session` state (`ReplSession`, `TransportView`, `PatternDisplayState`) while also handling direct IO logic (`run_stdio`, `run_with_handles`). This entangled core business state with IO processing.
**Blueprint:** Created `crates/orpheus-lang/src/session.rs` and moved the pure session state (`Session`, `TransportView`, `PatternDisplayState`) into it. Renamed `ReplSession` to `Session`. `repl.rs` and `tui.rs` now simply import and utilize this clean domain boundary.
