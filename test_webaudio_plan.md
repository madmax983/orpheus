1. **Scaffold Web Audio Exporter (Red Phase)**
   - Create `crates/orpheus-lang/src/webaudio_export.rs` with a failing test for exporting a number pattern to Web Audio HTML.
   - Include the module in `crates/orpheus-lang/src/lib.rs`.

2. **Implement Web Audio Exporter (Green Phase & Refactor)**
   - Implement `export_number_pattern_to_webaudio` to generate an HTML file with JavaScript that uses `AudioContext` to synthesize notes based on the pattern events.
   - Run tests to verify the test passes.
   - Ensure the generated code compiles cleanly without warnings.

3. **Complete Pre-Commit Verification**
   - Complete pre-commit steps to ensure proper testing, verification, review, and reflection are done.

4. **Submit PR as Nova**
   - Submit the PR matching Nova's guidelines (e.g., Title: "🌟 Nova: Web Audio Synthesizer Export", and a description containing "💡 The Spark", "🚀 The Feature", "🔮 The Potential", and "⚠️ Risk" sections).
