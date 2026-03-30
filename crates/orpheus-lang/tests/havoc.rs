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
    fn seq_sections_with_valid_items_does_not_allocate_oom(segments in 0u32..u32::MAX) {
        let source = format!("a = seq_sections(section(bd, {segments}))");
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
