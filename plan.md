1. **Discover & Scaffold:**
   - Evaluated the existing export functionalities. Orpheus already has integrations for several formats including SuperCollider, Lua, Sonic Pi, MIDI, HTML, Tracker, SVG, and ABC notations.
   - Designed a new exporter: a ChucK exporter. ChucK is a strongly-timed concurrent audio programming language that aligns perfectly with Orpheus's rational-time core and pattern composition, providing a unique textual export format.
   - Created `crates/orpheus-lang/src/chuck_export.rs`.
2. **Implement `export_sample_pattern_to_chuck`:**
   - Translates sample patterns by generating wait instructions (`{delta}::second => now;`) and buffer reading instructions (`"{sample}.wav" => buf.read; 0 => buf.pos;`).
3. **Implement `export_number_pattern_to_chuck`:**
   - Translates number patterns, assumed to be MIDI notes, into frequencies played by a `SinOsc`, writing `osc.freq` and `osc.gain` automation alongside wait instructions.
4. **Integrate into the public API:**
   - Add `pub(crate) mod chuck_export;` to `crates/orpheus-lang/src/lib.rs`.
   - Add `pub use chuck_export::{export_number_pattern_to_chuck, export_sample_pattern_to_chuck};` to `crates/orpheus-lang/src/lib.rs`.
5. **Add Tests:**
   - Added unit tests in `chuck_export.rs` for generating sample sequences and number sequences, validating that zero cycles return an `EvalError`, passing zero-cycle inputs.
6. **Verify everything works:**
   - Already done: ran `cargo clippy --package orpheus-lang --all-targets --all-features -- -D warnings`, `cargo fmt --all`, and `cargo test -p orpheus-lang -- chuck`.
7. **Complete pre-commit steps:**
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.
