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
        if let Ok(module) = module {
            if let Some(val) = module.get("notes") {
                // It should return an EvalError instead of triggering an OOM abort
                let _ = val.as_sample_pattern().unwrap().query_unit();
            }
        }
    }
}

/// 👺 Havoc: Unbounded recursion tests
#[test]
fn test_havoc_stack_overflow_unbounded_recursion() {
    // 🧨 The Trigger: The Omega Combinator! Evaluates to itself infinitely.
    // 📉 The Stack Trace: We avoid a stack overflow abort by checking evaluation depth limit.
    // 🧪 Reproduction: `cargo test test_havoc_stack_overflow_unbounded_recursion`
    // 😈 Comment: "You didn't expect people to write functional geometry in your music DSL."
    let source = "f x = x(x)\nomega = f(f)";
    let result = eval_module(&source, ReplMode::Loose);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("maximum evaluation depth exceeded"));
}
