1. Add an upper bound on `cycle_count` in `export_number_pattern_to_midi` and `export_sample_pattern_to_midi` to prevent allocating massive vectors (which results in OOM) when a user specifies an outrageously high cycle count for MIDI export.
   - Specifically, we'll check `cycle_count > 100_000` (similar to what's done in `tracker.rs` for `total_steps`).
2. Add a `#[test]` named `test_havoc_midi_export_cycle_limit` in `crates/orpheus-lang/src/midi_export.rs` that calls both functions with `cycle_count = 1_000_000` and asserts that it returns an `EvalError` about the maximum event limit.
3. Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
4. Submit PR.
