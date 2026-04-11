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

#[test]
fn havoc_omega_combinator_stack_overflow() {
    let source = r#"
f x = x(x)
a = f(f)
"#;
    let result = orpheus_lang::eval_module(source, orpheus_lang::ReplMode::Loose);
    assert_eq!(
        result.unwrap_err().to_string(),
        "maximum evaluation depth exceeded"
    );
}
