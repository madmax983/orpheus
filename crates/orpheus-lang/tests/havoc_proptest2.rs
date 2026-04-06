use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use proptest::prelude::*;

proptest! {
    /// 👺 Havoc: Test nested structures and large inputs to ensure no stack overflow or arithmetic panic
    #[test]
    fn test_havoc_eval_module_deep_nesting(depth in 1..200usize) {
        let mut source = String::new();
        source.push_str("a = ");
        for _ in 0..depth {
            source.push_str("fast(2, ");
        }
        source.push_str("bd");
        for _ in 0..depth {
            source.push(')');
        }
        let _ = eval_module(&source, ReplMode::Loose);
    }
}
