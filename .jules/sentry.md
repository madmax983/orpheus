## 2025-03-01 - Error types testing
**Learning:** Error types are often left out of coverage despite implementing important traits like `Display`. Testing these helps ensure the errors are readable to users.
**Action:** Add tests for diagnostic error types and their formatting.

## 2025-03-15 - Sample Manifest Parsing test coverage
**Learning:** The entire `crates/orpheus-dsp/src/sample_manifest.rs` parser was completely untested (0/253 lines covered), which is a high risk area as parsing external configuration directly handles user input.
**Action:** Add comprehensive parser unit tests targeting every conditional branch, duplicate checks, boundaries, and string literals.
## 2026-03-15 - Sentry Coverage Additions
**Learning:** Adding test coverage to pure data structures (like `EventStream` and `TimeSpan`) often involves ensuring edge cases like empty inputs, default trait implementations, out-of-bounds conditions, and clipping are explicitly tested. The `Pattern` trait on `EventStream` panics on `try_query` returning an Err, but that's practically unreachable as `clip_span` enforces bounds properly.
**Action:** Always test `Default` impls, empty data behaviors, partial interactions (clipping), and out-of-bounds boundary conditions.

## 2026-03-16 - More Sentry Coverage Additions
**Learning:** Sentry added tests validating 0 length behaviors and out of bounds clipping to EventStreams, expanding tests on structural primitives for greater coverage and compliance without mocking `TimeSpan` internally.
**Action:** Explicitly testing empty intervals using standard APIs yields better confidence than mock bypasses when standard types securely prevent incorrect states.

## 2024-03-08 - Testing Boundaries for Builders
**Learning:** Testing simple builder methods or structs without internal logic (like `SampleTrigger` builder) with extreme values provides no added value and violates testing boundaries.
**Action:** Do not test trivial getters/setters unless they contain explicit bounding, clipping, or initialization logic.

## 2024-03-18 - Missing Test Coverage in ActiveVoice
**Learning:** The `ActiveVoice` enum variants and the audio processing (DSP) filters `OnePoleLowPass` and `OnePoleHighPass` in `crates/orpheus-dsp/src/voice.rs` have 0% test coverage.
**Action:** Add tests for basic audio math components and simple voice generation boundaries since they represent core playback mechanics.
