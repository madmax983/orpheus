## 2023-10-27 - [OOM via Vec::with_capacity]
**Learning:** Pre-allocating a `Vec` based on an unverified user-supplied size constraint (like a header count) can lead to an Out-of-Memory (OOM) panic. Also, performing arithmetic (like `expected + 1`) on such an unverified value before capping it can result in an integer overflow panic in debug mode.
**Action:** Always safely clamp the capacity estimate (e.g., `.saturating_add(1).min(1024)`) so the collection falls back to standard dynamic resizing for extreme cases.

**[Extreme Float-to-Integer Saturation]**
**Learning:** In Rust, casting an out-of-bounds float to an integer directly (`value.round() as i128`) no longer causes undefined behavior, but it silently saturates to the boundary (`i128::MAX` or `MIN`). This defeats bounds checking if the check occurs *after* the cast, hiding garbage input from the system.
**Action:** Always perform strict boundary comparisons (`if value > std::i128::MAX as f64`) on the float type *before* attempting the `as` cast to prevent silent logical bugs.
## 2023-10-31 - [Tracker Export Emoji OOB Slice Panic]
**Learning:** Hardcoding string slice indexing like `&sample[..4]` causes panics when the boundary splits a multi-byte Unicode sequence (such as an emoji or complex character like `👹`).
**Action:** Use `.chars().take(4).collect::<String>()` instead of direct byte slicing for length-limited Unicode strings.
## 2023-10-31 - [Fuzzing Evaluation Resilience & Pattern Match Exhaustiveness]
**Learning:** `E0004: non-exhaustive patterns` compilation errors occur when adding new variants to central enums (like `BuiltinKind`) without updating matching functions downstream (`name()`, `arity()`, `execute()`). Fuzzing via `cargo-fuzz` confirmed the evaluation system handles malformed strings gracefully without crashing.
**Action:** When adding enum variants, systematically check and update all downstream match blocks. Ensure all systems compiling after a feature addition don't just compile but also withstand `cargo-fuzz` without panicking.
## 2025-02-28 - Float-to-integer conversion precision loss bypasses boundary check causing panics
**The Target:** `eval_positive_integer` float boundary check validation.
**The Finding:** The naive `value > i128::MAX as f64` check suffered from floating-point precision loss when validating extreme values (like `1.7014118346046923e38`), which resulted in the bound check silently passing and a subsequent panic (or saturation) during the `as i128` downcast.
**The Action:** Added a soft bound check `if value.abs() > 1.0e15_f64 { return Err(...); }` before performing precision-sensitive conversions.
