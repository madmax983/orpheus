use proptest::prelude::*;
use orpheus_lang::eval_module;
use orpheus_lang::ReplMode;
use std::panic;

proptest! {
    #[test]
    fn query_unit_sample_does_not_panic(s in any::<f64>()) {
        let source = format!("a = shift({s}, fast({s}, bd))");

        // This is strict: any panic triggered inside `eval_module` or `query_unit`
        // will cause the test to fail. `eval_module` doesn't evaluate the pattern span itself,
        // so we must do it manually via `query_unit()`.
        let result = panic::catch_unwind(|| {
            let res = eval_module(&source, ReplMode::Loose);
            if let Ok(mut values) = res {
                if let Some(val) = values.remove("a") {
                    if let Some(pat) = val.as_sample_pattern() {
                        let _ = pat.query_unit();
                    }
                }
            }
        });

        // If there was a panic, this assertion will fail.
        assert!(result.is_ok(), "query_unit panicked for input: {s}");
    }
}
