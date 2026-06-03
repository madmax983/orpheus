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

## R&D on Fuzzing & Mathematical Boundaries (Run)
* **Fuzzing and Concurrency:** Tested deeply nested AST structures (`fast(2, fast(2, bd))`) up to 1000 layers. The engine safely returns `maximum AST depth exceeded` instead of overflowing the stack.
* Investigated `Arc<Mutex<MidiOutputConnection>>` usage during `send`. Realized `send_midi_binding` spawns a background thread to handle `thread::sleep` offsets. Since the main thread drops the `Arc` rather than contending for the lock, a stalled MIDI send operation will only hang the background thread without blocking the user or halting the audio engine (no true deadlock).
* Fuzzed float casting limits and NaN precision boundaries in timing inputs (like `shift(0.0/0.0, ...)`). They are safely rejected by the Pest parser before they can trigger mathematical faults.

* **Mathematical Faults (Division-by-Zero):** Evaluated properties like `rem_euclid` using zero limits (`every(0, ...)`, `when(0, ...)`). The logic is well-guarded by `builtins.rs`, correctly trapping the zero and returning a handled `EvalError` ("requires a positive integer factor") instead of causing `i128` division-by-zero panics.
* Analyzed `div_euclid` in `apply_tuned_pitch_pattern` and `degrees`. Found that they assert array length and safely evaluate parse paths, returning descriptive `EvalError`s for invalid inputs (empty lists or boundary indices).

* **Conclusion:** The codebase exhibits excellent defensive programming boundaries. All mathematical boundary cases and extreme inputs fail securely by returning a `Result::Err` rather than panicking or deadlocking. I locked in these boundary conditions via explicit negative proptests in `crates/orpheus-lang/tests/fuzz_eval.rs` to ensure they never regress into unhandled faults.
