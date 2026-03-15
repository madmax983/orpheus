## 2025-03-01 - Error types testing
**Learning:** Error types are often left out of coverage despite implementing important traits like `Display`. Testing these helps ensure the errors are readable to users.
**Action:** Add tests for diagnostic error types and their formatting.

## 2025-03-15 - Sample Manifest Parsing test coverage
**Learning:** The entire `crates/orpheus-dsp/src/sample_manifest.rs` parser was completely untested (0/253 lines covered), which is a high risk area as parsing external configuration directly handles user input.
**Action:** Add comprehensive parser unit tests targeting every conditional branch, duplicate checks, boundaries, and string literals.
