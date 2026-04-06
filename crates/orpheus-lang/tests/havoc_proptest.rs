use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use proptest::prelude::*;

proptest! {
    /// 👺 Havoc: Throws arbitrary garbage strings at the evaluator to ensure
    /// that nothing panics (e.g. index out of bounds, unwrap on None, or stack overflow)
    /// during parsing or evaluation. All failures should be returned gracefully as an Error.
    #[test]
    fn test_havoc_eval_module_garbage_input(source in "\\PC*") {
        let _ = eval_module(&source, ReplMode::Loose);
        let _ = eval_module(&source, ReplMode::Strict);
    }
}
