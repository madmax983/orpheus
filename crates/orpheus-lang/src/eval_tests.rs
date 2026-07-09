#[cfg(test)]
mod havoc_tests {
    use crate::ReplMode;
    use crate::eval::{eval_module, f64_to_rational};
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_havoc_float_literal_parse_fuzz(s in any::<f64>()) {
            let source = format!("a = {}", s);
            let _ = eval_module(&source, ReplMode::Loose);
        }

        #[test]
        fn test_havoc_fast_shift(s in any::<f64>()) {
            let source = format!("a = shift({}, fast({}, bd))", s, s);
            let _ = eval_module(&source, ReplMode::Loose);
        }

        #[test]
        fn test_havoc_apply_user_function_args(args in proptest::collection::vec(any::<f64>(), 0..10)) {
            let args_str = args.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ");
            let source = format!("f x = x\nres = f({})", args_str);
            let _ = eval_module(&source, ReplMode::Loose);
        }

        #[test]
        fn test_havoc_f64_to_rational_proptest(v in any::<f64>()) {
            let _ = f64_to_rational(v, "test");
        }
    }
}
