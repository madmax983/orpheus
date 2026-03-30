👺 Havoc: Concurrent torn reads and integer overflows

🧨 **The Trigger:**
A concurrent transport synchronization issue was caught where `compiler_fence` semantics were incorrectly assumed, leading to torn reads under high thread preemption.
In `orpheus-lang`, fuzz testing revealed that `parse_named_pitch_literal` had no bounds checking on very long integer inputs leading to panic during octave parsing.
In `orpheus-pattern`, proptests revealed that safe arithmetic on Rational values is not crashing nor overflowing panic.

📉 **The Stack Trace:**
Thread panicked with `ParseIntError: overflow`.
Torn reads under concurrent tests with Loom model preemption.

🧪 **Reproduction:**
Run `cargo test -p orpheus-pattern --test havoc`.
Run `cargo test -p orpheus-lang --test havoc`.
Run `LOOM_MAX_PREEMPTIONS=2 RUSTFLAGS="--cfg loom" cargo test -p orpheus-dsp --test havoc`

😈 **Comment:**
"You assumed memory fences didn't need Acquire/Release ordering. You were wrong."
"You assumed no one would ever type c99999999999999999. You were wrong."
"You assumed absolute certainty is enough without fuzzing. You were wrong."
