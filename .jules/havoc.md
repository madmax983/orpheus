## 2023-10-27 - [OOM via Vec::with_capacity]
**Learning:** Pre-allocating a `Vec` based on an unverified user-supplied size constraint (like a header count) can lead to an Out-of-Memory (OOM) panic. Also, performing arithmetic (like `expected + 1`) on such an unverified value before capping it can result in an integer overflow panic in debug mode.
**Action:** Always safely clamp the capacity estimate (e.g., `.saturating_add(1).min(1024)`) so the collection falls back to standard dynamic resizing for extreme cases.
**[Panic Vulnerability Check]**
**Learning:** The system has comprehensive safeguards against panic paths in AST evaluation and string manipulation. Loom tests and Fuzzer tests all pass with bounds-checking and OOM saturation protections in place.
**Action:** Submit PR to document the successful fuzzing limits and add our new positive Havoc tests for regression catching.
