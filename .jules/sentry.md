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

## 2026-03-18 - Type Format and Constructor Test Coverage
**Learning:** Pure enum representations of abstract types like `Type` often lack coverage on their `Display` implementations and constructor methods (`pattern`, `function`, `curried`), hiding potential formatting bugs when type errors are reported to users.
**Action:** Ensure type representations and ASTs have unit tests verifying their debug/display string outputs and builder patterns.

## 2026-03-19 - Type annotations for generic tests
**Learning:** When creating empty generic structs (like `CyclePattern::from_nodes(vec![])`) in test cases, the compiler will error out with `E0282: type annotations needed` because it lacks context to infer `T`.
**Action:** Always provide explicit type bounds (e.g., `let pattern: CyclePattern<&str> = ...`) when instantiating empty generic containers for tests.
