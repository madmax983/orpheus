🛡️ Sentry: [test coverage improvement]

🎯 Target: `PluginHostError`, `PluginDescriptor`, `PluginParameterLane`, `PluginNote` inside `plugin_host.rs` and `cycle.rs`, `stream.rs` inside `orpheus-pattern`.
💣 Risk: Edge cases related to headless configuration and arithmetic overflow during evaluations.
🧪 Strategy: Added specific unit tests to ensure that invalid bounds fail early when constructing configurations for `vst3`/`au` configurations, covered empty bounds errors for paths, and verified `try_query` branches.
🔬 Verification: Run `cargo test -p orpheus-dsp --test plugin_hosting` and `cargo test -p orpheus-pattern`
