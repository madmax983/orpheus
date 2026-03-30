use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;
use std::time::Instant;

#[test]
fn havoc_seq_sections_timeout() {
    let source = "a = seq_sections(section(bd, 9999999))";

    let start = Instant::now();
    let result = eval_module(&source, ReplMode::Loose);
    let duration = start.elapsed();

    if duration.as_secs() > 2 {
        panic!("💥 DETONATED: Evaluation took too long, likely due to an unbounded loop!");
    }

    let err = result.unwrap_err();
    assert!(
        err.to_string()
            .contains("exceeded the maximum allowed bound")
    );
}
