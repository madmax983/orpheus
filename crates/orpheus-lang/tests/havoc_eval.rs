use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_havoc_capacity_overflow_eval(count in 1_000_000..usize::MAX) {
        // We use a large count that would otherwise trigger checked_mul to overflow
        // or trigger an out of memory panic due to unwrap_or(0).
        let source = format!("notes = [60]*{count}");
        let _ = eval_module(&source, ReplMode::Loose);
        // It should return an EvalError, not panic!
    }
}
