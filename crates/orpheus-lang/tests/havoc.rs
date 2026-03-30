use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use proptest::prelude::*;
use std::panic;

proptest! {
    #[test]
    fn havoc_proptest_named_pitch_literal(s in "[a-g][sf]?[0-9]*") {
        let source = format!("x = n(\"{}\")", s);
        let _ = eval_module(&source, ReplMode::Loose);
    }
}

proptest! {
    #[test]
    fn eval_named_pitch_literal_does_not_panic(s in "[A-Za-z0-9_-]*") {
        let source = format!("a = n(\"{}\")", s);
        let _ = panic::catch_unwind(|| {
            let _ = eval_module(&source, ReplMode::Loose);
        });
    }
}

proptest! {
    #[test]
    fn havoc_proptest_eval(s in ".*") {
        let _ = panic::catch_unwind(|| {
            let _ = eval_module(&s, ReplMode::Loose);
        });
    }
}
