use orpheus_lang::{ReplMode, eval_module};
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_havoc_proptest_math_funcs(
        a in any::<f64>(),
        b in any::<f64>(),
    ) {
        if a.is_finite() && b.is_finite() {
            let src1 = format!("res = shift({a}, {b})");
            let _ = eval_module(&src1, ReplMode::Loose);
            let src2 = format!("res = fast({a}, {b})");
            let _ = eval_module(&src2, ReplMode::Loose);
        }
    }
}
