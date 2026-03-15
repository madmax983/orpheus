## 2025-03-01 - Error types testing
**Learning:** Error types are often left out of coverage despite implementing important traits like `Display`. Testing these helps ensure the errors are readable to users.
**Action:** Add tests for diagnostic error types and their formatting.
## 2026-03-15 - Sentry Coverage Additions
**Learning:** Adding test coverage to pure data structures (like `EventStream` and `TimeSpan`) often involves ensuring edge cases like empty inputs, default trait implementations, out-of-bounds conditions, and clipping are explicitly tested. The `Pattern` trait on `EventStream` panics on `try_query` returning an Err, but that's practically unreachable as `clip_span` enforces bounds properly.
**Action:** Always test `Default` impls, empty data behaviors, partial interactions (clipping), and out-of-bounds boundary conditions.
