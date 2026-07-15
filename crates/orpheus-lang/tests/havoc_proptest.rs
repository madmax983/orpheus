use orpheus_lang::{ReplMode, eval_module};
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_havoc_proptest_number_bounds(val in 1.0e15f64..1.0e40f64) {
        // Attempt to cause out of bounds bypass by injecting large precision-lost floats
        // that surpass 1.0e15 into contexts validated by `eval_positive_integer`.
        // e.g., the section cycle count in `meter`
        // We write meter(val, 4, bd).
        // Since `eval_positive_integer` checks bounds directly against `i128::MAX as f64`,
        // it may silently cast/truncate extremely large numbers unless bounded earlier.
        // It SHOULD error out with "exceeded the supported range".

        let source = format!("x = meter({}, 4, bd)", val);
        let res = eval_module(&source, ReplMode::Loose);
        assert!(
            res.is_err(),
            "Expected error for large float bounds bypass, but evaluation succeeded! Float value: {}", val
        );
        let err_msg = res.unwrap_err().to_string();

        assert!(
            err_msg.contains("exceeded the supported range"),
            "Expected 'exceeded the supported range', got: {}", err_msg
        );
    }
}
