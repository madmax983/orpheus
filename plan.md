1. **Smell:** `apply_every` uses a verbose `match` block to route between `SamplePattern` and `NumberPattern`, duplicating error handling logic.
2. **Solution:** Refactor `apply_every` to use the `apply_pattern_transform` helper function, similar to `apply_when`, `apply_jux`, and `apply_within`.
3. **Benefit:** Reduces duplication, flattens the structure, and ensures consistent error messaging across built-in pattern transforms.
4. **Verification:** Run `cargo clippy`, `cargo fmt`, and `cargo test` to ensure tests pass and logic is unchanged. Add a journal entry to `.jules/forge.md` documenting this refactor.
