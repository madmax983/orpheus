//! Fuzz testing the pattern evaluation engine for edge-case panics and evaluator limits.

use orpheus_lang::{ReplMode, eval_module};
use proptest::prelude::*;
use std::panic;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn test_havoc_eval_module_fuzz(s in ".*") {
        // Havoc: Inject garbage text of any length into the evaluator.
        // Orpheus' pest grammar and AST traversal should never panic.
        let result = panic::catch_unwind(|| {
            let _ = eval_module(&s, ReplMode::Loose);
        });
        assert!(result.is_ok(), "eval_module panicked for input: {s}");
    }
}
