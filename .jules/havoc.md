## 2025-05-24 - Unreachable match arm
**The Trigger:** A duplicate match pattern in `orpheus-lang/src/builtins.rs` caused an unreachable code compilation warning/error under `-D warnings`.
**The Stack Trace:** compiler error
**Reproduction:** cargo test or cargo clippy
**Comment:** We can't let simple warnings break our build.

## 2025-05-24 - Filter State Bug
**The Trigger:** A bad logic implementation and test in `should_process_comb_filter` or `process` function logic in `reverb.rs`. The code was modified by someone else but `tests` were broken.
**The Stack Trace:** Panic due to assertion left == right
**Reproduction:** cargo test -p orpheus-dsp
**Comment:** A recent commit broke the filter state logic.

## 2025-05-24 - Fuzz tests robustness
**The Trigger:** Fuzz testing `eval_module`
**The Stack Trace:** none
**Reproduction:** run `proptest`
**Comment:** The code handles parsing unexpected numbers and characters gracefully by returning `Err` values instead of panicking.
