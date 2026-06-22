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
## 2026-06-22 - [Havoc: AST Fuzzing & Stress Testing boundaries]
**The Trigger:** Maliciously deeply nested code like `stream(stream(stream(...)))` and garbage byte injections into the MIDI input parser (`update_from_message([0xFF, 0x00...])`).
**The Stack Trace:** No panic occurred! The code successfully rejected invalid structures via `Result::Err` and safely discarded incomplete MIDI payloads. However, to formally verify this under intense mutation, we added explicit proptests.
**Reproduction:** Run `cargo test -p orpheus-lang --test fuzz_eval` and `cargo test -p orpheus-lang --lib midi_input`.
**Comment:** The codebase is remarkably robust against AST evaluation stack limits and basic fuzzing. We formally encoded these assertions into chaos tests so regression bots catch future weaknesses.
