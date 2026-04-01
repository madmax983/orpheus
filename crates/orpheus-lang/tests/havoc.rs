use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use proptest::prelude::*;

proptest! {
    #[test]
    fn euclid_does_not_allocate_oom(pulses in 0u32..u32::MAX, steps in 0u32..u32::MAX) {
        let source = format!("a = euclid({pulses}, {steps})");
        let _ = eval_module(&source, ReplMode::Loose);
    }
}

proptest! {
    #[test]
    fn when_does_not_allocate_oom(offset in 0u32..u32::MAX) {
        let source = format!("a = when({offset}, rev, bd)");
        let _ = eval_module(&source, ReplMode::Loose);
    }
}

proptest! {
    #[test]
    fn seq_sections_does_not_allocate_oom(segments in 0u32..u32::MAX) {
        let source = format!("a = seq_sections({segments}, bd, sn)");
        let _ = eval_module(&source, ReplMode::Loose);
    }
}

proptest! {
    #[test]
    fn fuzz_math_extreme_vals_add(a in proptest::num::f64::ANY, b in proptest::num::f64::ANY) {
        let s = format!("x = {} + {}", a, b);
        let _ = eval_module(&s, ReplMode::Loose);
    }

    #[test]
    fn fuzz_math_extreme_vals_mul(a in proptest::num::f64::ANY, b in proptest::num::f64::ANY) {
        let s = format!("x = {} * {}", a, b);
        let _ = eval_module(&s, ReplMode::Loose);
    }

    #[test]
    fn fuzz_run_negative_extreme(count in proptest::num::i64::ANY) {
        let s = format!("x = run(-{})", count.saturating_abs());
        let _ = eval_module(&s, ReplMode::Loose);
    }
}
