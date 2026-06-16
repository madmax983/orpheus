use orpheus_lang::{eval_module, parse_module, parse_scala_source, ReplMode};
use proptest::prelude::*;

proptest! {
    #[test]
    fn havoc_test_parse_scala_source(s in "\\PC*") {
        let _ = parse_scala_source(&s, "fuzz");
    }

    #[test]
    fn havoc_test_parse_module(s in "\\PC*") {
        let _ = parse_module(&s);
    }

    #[test]
    fn havoc_test_eval_module_dos_repeat(count in 1..=10_000u32) {
        let source = format!("pat = [bd]{{{count}}}");
        let _ = eval_module(&source, ReplMode::Loose);
    }

    #[test]
    fn havoc_test_eval_module_fuzzing(ref source in "\\PC*") {
        let _ = eval_module(source, ReplMode::Loose);
    }
}

#[test]
fn havoc_eval_module_dos_large_section_cycle() {
    let source = "pat = section(bd, 1000000)";
    let result = eval_module(source, ReplMode::Loose);
    assert!(result.is_err(), "Expected an error due to evaluator limits");
}

#[test]
fn havoc_eval_module_dos_deep_recursion() {
    use std::fmt::Write;
    let mut source = String::from("f x = x\n");
    for i in 0..1000 {
        let _ = writeln!(source, "x{i} = f(bd)");
    }
    let result = eval_module(&source, ReplMode::Loose);
    assert!(result.is_ok() || result.is_err()); // Ensure no panic
}
