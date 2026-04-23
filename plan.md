We've spent a significant amount of time attacking `orpheus` looking for panics, deadlocks, and array out of bounds vulnerabilities. We used property testing with proptest and the libfuzzer based cargo-fuzz. The codebase handles unexpected states gracefully through Result/Option without panics. We tested:
- MIDI queue handling with negative offsets and extreme sleep offsets.
- Number literals with huge decimal values and unparseable trailing zeros.
- Large repeating sequence and deeply nested fast/slow nodes mapping AST depth parsing and evaluation layers.
- Random bytes mapping the `eval_module` function.
- String slicing in `pitch.rs` using UTF-8 bounds.

No panic was encountered. This is considered a success under Havoc persona since we can document the extreme lengths we went to find vulnerabilities and prove the codebase is robust under these areas.

Pre-commit steps will run formatting and checking. We'll leave the test files behind as "wreckage" proofs.
