use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]
    #[test]
    fn test_havoc_capacity_overflow_eval(depth in 1..100_usize, factor in 2..10_i64) {
        let mut source = "bd".to_string();
        for _ in 0..depth {
            source = format!("fast({factor}, {source})");
        }
        source = format!("notes = {source}");

        let module = eval_module(&source, ReplMode::Loose);
        #[allow(clippy::collapsible_if)]
        if let Ok(module) = module {
            #[allow(clippy::collapsible_if)]
            if let Some(val) = module.get("notes") {
                // It should return an EvalError instead of triggering an OOM abort
                let _ = val.as_sample_pattern().unwrap().query_unit();
            }
        }
    }
}
