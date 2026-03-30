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
fn havoc_eval_module_sequence_stack_depth() {
    use orpheus_lang::parse_module;
    let depth = 500;
    let mut code = String::from("x = ");
    for _ in 0..depth {
        code.push_str("fast(2, [");
    }
    code.push_str("bd");
    for _ in 0..depth {
        code.push(']');
        code.push(')');
    }

    let result = parse_module(&code);
    match result {
        Ok(_ast) => {
            panic!("Should have failed to parse deeply nested pattern");
        }
        Err(e) => {
            assert!(e.to_string().contains("nesting depth exceeded maximum"));
        }
    }
}
