⚡ Bolt: Uses `sort_unstable_by` over `sort_by`

💡 What: Swapped `sort_by` and `sort_by_key` with `sort_unstable_by` and `sort_unstable_by_key` in several places where we were sorting arrays without explicitly requiring stability.
🎯 Why: `sort_by` allocates memory to ensure stable relative ordering which can increase allocation overhead on hot paths unnecessarily. `sort_unstable_by` is an in-place sort that provides the same output when stability isn`t strictly needed.
📊 Impact: Removes heap allocations across hot paths such as event parsing streams and sample bank loads.
🔬 Measurement: Run `cargo test` and `cargo bench` to confirm correctness.
