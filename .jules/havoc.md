## 2023-10-27 - [OOM via Vec::with_capacity]
**Learning:** Pre-allocating a `Vec` based on an unverified user-supplied size constraint (like a header count) can lead to an Out-of-Memory (OOM) panic. Also, performing arithmetic (like `expected + 1`) on such an unverified value before capping it can result in an integer overflow panic in debug mode.
**Action:** Always safely clamp the capacity estimate (e.g., `.saturating_add(1).min(1024)`) so the collection falls back to standard dynamic resizing for extreme cases.

**[Havoc Resilience]**
**Learning:** The Orpheus system has been proven highly resilient against fuzzing and property testing on core string parsers (`eval_module`, `eval_into_bindings`, `f64_to_rational`, `parse_named_pitch_literal`, `escape_json_string`, `parse_scala_source`), MIDI concurrency (`Mutex`/`AtomicU8`), and AST recursion, properly returning `Result::Err` or maintaining stable state instead of panicking or crashing.
**Action:** Avoid redundant chaos testing on these specific validated endpoints unless significant internal structural changes are made. The robust limits have been documented via passing boundary tests.
