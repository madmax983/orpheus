use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use proptest::prelude::*;

proptest! {
    /// 👺 Havoc: Tests that early returns in the evaluator do not leak the recursion depth state
    #[test]
    fn test_havoc_eval_early_return_leak(s in ".*") {
        let source = format!("a = shift(4, slow(3, fast(1.0e+90, every(4, rev, sine))))\nx = {s}\ny = x |> 2\nz = notes");
        let _ = eval_module(&source, ReplMode::Loose);
        // After an evaluation that likely fails and returns early via `?`,
        // evaluating a simple expression should still succeed and not hit the recursion limit.
        let result = eval_module("a = 1", ReplMode::Loose);
        assert!(result.is_ok(), "State leak: evaluator depth was not restored after early return.");
    }
}
