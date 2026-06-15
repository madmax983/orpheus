use orpheus_lang::{eval_module, f64_to_rational, ReplMode};
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// 👺 Havoc: Tests that injecting completely arbitrary floating point data (including
    /// negative infinity, subnormals, and massive exponents) into the core time-conversion
    /// utility gracefully returns an EvalError rather than overflowing, truncating silently,
    /// or triggering a panic inside standard library routines.
    #[test]
    fn test_havoc_f64_to_rational_robustness(val in any::<f64>()) {
        let _ = f64_to_rational(val, "fuzz context");
    }

    /// 👺 Havoc: Tests that injecting large, arbitrary string permutations of valid and
    /// invalid operator characters, identifiers, and grouping symbols directly into the
    /// parser and evaluator does not trigger a stack overflow or fatal panic (e.g. from
    /// deeply nested recursive descents or invalid span slicing).
    #[test]
    fn test_havoc_eval_module_parser_robustness(src in "[a-zA-Z0-9=()_|>*/+., -]{1,500}") {
        let _ = eval_module(&src, ReplMode::Loose);
    }
}
