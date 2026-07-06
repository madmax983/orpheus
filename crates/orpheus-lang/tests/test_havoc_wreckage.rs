use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use proptest::prelude::*;

// 👺 Havoc: Wreckage tests. These tests are the remains of our chaos engineering.
// The system has successfully proven to handle these edge cases without panicking,
// deadlocking, or running out of memory. They are preserved here as a testament to its resilience.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn test_havoc_division_by_zero_proptest_eval(s in "seq_sections\\(section\\(bd, 0\\)\\)") {
        let _ = eval_module(&s, ReplMode::Loose);
    }

    #[test]
    fn test_havoc_at_infinity(time in any::<f64>()) {
        let source = format!("notes = meter(4, 4, at({time}, bd))");
        let _ = eval_module(&source, ReplMode::Loose);
    }

    #[test]
    fn test_havoc_fuzz_meter_arguments(
        beats in any::<f64>(),
        unit in any::<f64>()
    ) {
        let source = format!("notes = meter({beats}, {unit}, bd)");
        let _ = eval_module(&source, ReplMode::Loose);
    }
}
