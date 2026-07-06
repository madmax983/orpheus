1. **Refactor `parse_region` in `crates/orpheus-dsp/src/sample_manifest.rs`**
   - Address the code review feedback: the helper `parse_region_field` takes too many mutable arguments. I should extract these into a Context struct to follow Forge's rule: "Struct Extraction: If a function takes 5 arguments, turn them into a Context struct."
   - Create a `RegionContext` struct to hold `token`, `start`, `end`, `rate`, and `seen_rate`.
   - Implement `parse_region_field` as a method on `RegionContext` taking `&mut self`.
   - This will dramatically clean up the code.

2. **Verify the changes compile and tests pass.**
   - Run `cargo clippy --all-targets --all-features -- -D warnings`.
   - Run `cargo test` and verify that `test_sample_manifest` integration test passes, as well as the unit tests in the file.
   - Run `cargo fmt --all`.

3. **Complete pre-commit steps**
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.

4. **Submit the change.**
   - Use `submit` with branch name `forge-refactor-sample-manifest-parser`
   - Title: '⚒️ Forge: Refactor parse_region to eliminate God Function'
   - Description containing 🚮 Smell, ✨ Solution, 🧼 Benefit, 🛡️ Verification.
