**Learning:** `PluginPatternValue`, `PitchClassSetValue`, and `ArpDirectionValue` in `orpheus-lang/src/value.rs` lacked `Explain` implementations, falling back to a raw `Value` missing case and unformatted errors in the REPL.
**Action:** Always verify new values added to `Value` have corresponding `Explain` implementations.
