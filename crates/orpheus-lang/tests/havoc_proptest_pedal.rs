use orpheus_lang::{ReplMode, eval_module};
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_havoc_proptest_pedal_nodes(
        a in any::<f64>(),
        b in any::<f64>(),
    ) {
        if a.is_finite() && b.is_finite() {
            let src1 = format!("res = graph {{ x = {a}; y = {b}; x + y }}");
            let _ = eval_module(&src1, ReplMode::Loose);
            let src2 = format!("res = graph {{ x = {a}; y = {b}; x * y }}");
            let _ = eval_module(&src2, ReplMode::Loose);
        }
    }
}
