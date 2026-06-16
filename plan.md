1. **Refactor `tracker.rs` export grids**
   - In `crates/orpheus-lang/src/tracker.rs`, extract the `grid` population logic to private helper methods to simplify `export_sample_pattern_to_tracker` and `export_number_pattern_to_tracker`.
2. **Refactor `value.rs` massive match statements to remove `clippy::too_many_lines`**
   - The method `with_tuning` has an `#[allow(clippy::too_many_lines)]`. It's a huge match statement over `PatternRuntime`. I'll split it by extracting parts of the match arms into helper methods like `with_tuning_audio_effect`, `with_tuning_modulation_effect`, similar to what's done for `try_query`.
3. **Refactor `tui/plugins.rs` large `render` function**
   - `crates/orpheus-lang/src/tui/plugins.rs` has a `render` function in `pub struct TranscriptPlugin` that is fairly large and could be split into helpers like `render_transcript_lines`, `render_status_messages`.
4. Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
5. Create PR with title "⚒️ Forge: Refactor tracker, value, and tui plugins for better readability"
