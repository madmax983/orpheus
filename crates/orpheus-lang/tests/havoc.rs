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
    fn shift_does_not_panic(offset in f64::MIN..f64::MAX) {
        let source = format!("a = shift({offset}, bd)");
        let _ = eval_module(&source, ReplMode::Loose);
    }
}

proptest! {
    #[test]
    fn rand_does_not_panic(seed in 0u64..u64::MAX) {
        let source = format!("a = rand() |> fast({seed})");
        let _ = eval_module(&source, ReplMode::Loose);
    }
}

proptest! {
    #[test]
    fn hpf_lpf_do_not_panic(cutoff in f64::MIN..f64::MAX) {
        let source_hpf = format!("a = hpf({cutoff}, bd)");
        let _ = eval_module(&source_hpf, ReplMode::Loose);

        let source_lpf = format!("a = lpf({cutoff}, bd)");
        let _ = eval_module(&source_lpf, ReplMode::Loose);
    }
}
