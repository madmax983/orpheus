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

#[test]
fn havoc_parser_depth_explosion() {
    // Generate a deeply nested AST to exploit backtracking / stack recursion.
    // The fuzzer found inputs like `stack(((((...` took 43 seconds to evaluate!
    let mut source = String::from("a = ");
    for _ in 0..10_000 {
        source.push('(');
    }
    source.push_str("bd");
    for _ in 0..10_000 {
        source.push(')');
    }

    let start = Instant::now();
    // We expect it to either stack overflow (aborting the test runner entirely)
    // or take an unreasonable amount of time (triggering the duration check).
    let result = eval_module(&source, ReplMode::Loose);
    let duration = start.elapsed();

    assert!(
        duration.as_secs() <= 2,
        "💥 DETONATED: Parser took too long to evaluate deeply nested input, likely due to catastrophic backtracking or unbounded recursion!"
    );

    if let Err(err) = result {
        // Assert we fail fast with a nice depth limit rather than taking minutes.
        assert!(
            err.to_string().contains("exceeded maximum AST depth")
                || err.to_string().contains("parse error")
        );
    }
}
