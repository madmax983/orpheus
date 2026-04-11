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
        let Ok(module) = module else { return Ok(()); };
        let Some(val) = module.get("notes") else { return Ok(()); };

        // It should return an EvalError instead of triggering an OOM abort
        let _ = val.as_sample_pattern().unwrap().query_unit();
    }
}
