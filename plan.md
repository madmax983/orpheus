1. Add `impl Explain for PluginPatternValue` in `crates/orpheus-lang/src/value.rs`.
   - The implementation will use `explain_table` and `comfy-table` to present the plugin's details, such as the type ("Headless Plugin Instrument"), format (e.g., "VST3", "AudioUnit"), identifier, and the number of parameter lanes and note events.
2. Update the `:explain` command handling in `crates/orpheus-lang/src/session.rs`.
   - In `explain_binding`, add a match arm for `crate::value::Value::PluginPattern(plugin) => Ok(plugin.explain(binding_name))`.
3. Add tests to ensure `:explain` works correctly for plugin bindings.
   - Update `crates/orpheus-lang/tests/plugin_hosting.rs` or create a new test in `crates/orpheus-lang/src/session.rs` to verify the output formatting and that it doesn't return an error.
4. Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
5. Create a PR reflecting Mosaic's visual polish for the Plugin pattern explain UI.
