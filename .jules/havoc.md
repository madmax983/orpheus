## 2023-10-27 - [OOM via Vec::with_capacity]
**Learning:** Pre-allocating a `Vec` based on an unverified user-supplied size constraint (like a header count) can lead to an Out-of-Memory (OOM) panic. Also, performing arithmetic (like `expected + 1`) on such an unverified value before capping it can result in an integer overflow panic in debug mode.
**Action:** Always safely clamp the capacity estimate (e.g., `.saturating_add(1).min(1024)`) so the collection falls back to standard dynamic resizing for extreme cases.
## 2025-02-28 - [Scheduler Vec OOM]
**Learning:** Using `Vec::with_capacity` directly with an unverified iterator `size_hint` (e.g. from an AST pattern iterator that could be maliciously sized or infinite) can cause an immediate out-of-memory panic, crashing the `orpheus-dsp` scheduling loop.
**Action:** Always clamp the maximum allocation size (e.g. `.min(1024)`) so that `Vec` dynamically resizes for valid inputs rather than trusting the upper bound.
