#!/bin/bash

# Restore the fast/slow float math op fuzz tests that got lost in reset --hard
cat << 'FUZZ' >> crates/orpheus-lang/tests/fuzz_eval.rs

proptest! {
    #[test]
    fn no_panic_on_f64_math_ops(v1 in proptest::num::f64::ANY, v2 in proptest::num::f64::ANY) {
        let source = format!("x = fast({} * {}, bd)", v1, v2);
        let _ = eval_module(&source, ReplMode::Loose);
        let source = format!("x = fast({} / {}, bd)", v1, v2);
        let _ = eval_module(&source, ReplMode::Loose);
        let source = format!("x = fast({} + {}, bd)", v1, v2);
        let _ = eval_module(&source, ReplMode::Loose);
        let source = format!("x = fast({} - {}, bd)", v1, v2);
        let _ = eval_module(&source, ReplMode::Loose);
        let source = format!("x = fast({} % {}, bd)", v1, v2);
        let _ = eval_module(&source, ReplMode::Loose);
        let source = format!("x = fast({} ^ {}, bd)", v1, v2);
        let _ = eval_module(&source, ReplMode::Loose);
    }
}
FUZZ
