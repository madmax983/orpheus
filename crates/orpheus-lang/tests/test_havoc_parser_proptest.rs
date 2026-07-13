use orpheus_lang::{ReplMode, eval_module};
use proptest::prelude::*;

proptest! {
    // 👺 Havoc: Fuzzing the evaluator directly with completely unconstrained,
    // random string input to find structural panics or recursive stack overflow bypasses.
    #[test]
    fn test_havoc_proptest_ast(s in ".*") {
        // Any error is a safe outcome as long as it doesn't panic.
        let _ = eval_module(&s, ReplMode::Loose);
    }
}
