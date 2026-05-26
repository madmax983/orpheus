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

**[Proving System Resilience with Chaos Tests]**
**The Trigger:** Extremely large lengths (OOM), cyclic recursion boundaries, negative timing parameters, non-finite values in DSP routines (NaN/Infinity), and random byte garbage parsing.
**The Stack Trace:** Initially produced recursive evaluation stack overflows, test suite OOM allocations for tracker rendering, parser crashes on unicode boundaries, and floating-point errors propagating into invalid rational allocations.
**Reproduction:** Run `cargo fuzz run fuzz_target_1`, or `cargo test --test havoc_proptest_crash`.
**Comment:** We built extensive tests proving boundaries limit scaling allocation (100,000 max tracker frames), evaluation halts on 200 recursion frames, `loom` thread tests show zero data races across `ReplSession::eval_line`, and all public numeric APIs cleanly propagate out-of-range floats rather than panicking on conversion. The system withstood the chaos.
