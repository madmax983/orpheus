use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use std::time::Instant;

#[test]
fn havoc_seq_sections_timeout() {
    let source = "a = seq_sections(section(bd, 9999999))";

    let start = Instant::now();
    let result = eval_module(source, ReplMode::Loose);
    let duration = start.elapsed();

    assert!(
        duration.as_secs() <= 2,
        "💥 DETONATED: Evaluation took too long, likely due to an unbounded loop!"
    );

    let err = result.unwrap_err();
    assert!(
        err.to_string()
            .contains("exceeded the maximum allowed bound")
    );
}

use proptest::prelude::*;
use std::panic;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10000))]
    #[test]
    fn havoc_eval_does_not_panic(s in "\\PC*") {
        let result = panic::catch_unwind(|| {
            let _ = eval_module(&s, ReplMode::Loose);
        });

        assert!(result.is_ok(), "💥 DETONATED: eval_module panicked on input: {:?}", s);
    }
}
