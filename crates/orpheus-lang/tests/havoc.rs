use orpheus_lang::eval_module;
use orpheus_lang::ReplMode;

#[test]
fn havoc_fast_oom() {
    let source = "a = fast(10000000, bd)";

    // We expect eval_module to return an error because the factor is too large.
    // The test asserts that `eval_module` returns an Err.
    let result = eval_module(source, ReplMode::Loose);

    assert!(result.is_err(), "Vulnerability: extremely large `fast` factor was accepted, causing massive memory allocations in query_unit()");
}
