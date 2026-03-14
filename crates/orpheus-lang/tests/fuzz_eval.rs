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
            let mut values = eval_module(&source, ReplMode::Loose)
                .expect("eval_module failed");
            let val = values
                .remove("a")
                .expect("binding `a` not found");
            let pat = val
                .as_sample_pattern()
                .expect("value `a` is not a sample pattern");
            let _ = pat.query_unit();
        });

        // If there was a panic, this assertion will fail.
        assert!(result.is_ok(), "query_unit panicked for input: {s}");
    }
}
