use std::panic;
use proptest::proptest;
use orpheus_lang::{eval_module, ReplMode};

proptest! {
    #[test]
    fn slice_idx_does_not_panic(segments in 0u32..u32::MAX) {
        let source = format!("a = slice_idx(0, {segments}, bd sn)");
        let result = panic::catch_unwind(|| {
            let mut env = match eval_module(&source, ReplMode::Loose) {
                Ok(env) => env,
                Err(_) => return, // Ignore parse/eval errors for fuzz testing
            };
            let Some(pat) = env.remove("a") else { return; };
            let Some(sample_pat) = pat.as_sample_pattern() else { return; };
            let _ = sample_pat.query_unit();
        });
        assert!(result.is_ok());
    }
}

proptest! {
    #[test]
    fn rand_shift_nested_does_not_panic(_s in 0u64..u64::MAX) {
        let source = format!("a = shift(rand(), slow(2, fast(3, bd sn)))");
        let result = panic::catch_unwind(|| {
            let mut env = eval_module(&source, ReplMode::Loose).unwrap();
            let pat = env.remove("a").unwrap();
            let _ = pat.as_sample_pattern().unwrap().query_unit();
        });
        assert!(result.is_ok());
    }
}
