//! Tests evaluating euclidean rhythm generation with edge-case parameters like zero or negative steps to prevent division-by-zero panics.
use orpheus_lang::ReplMode;
use orpheus_lang::eval_module;

#[test]
fn test_havoc_rem_euclid_zero_every() {
    let source = "a = every(0.00000000000000001, rev, bd)\nnotes = a";
    let env = eval_module(source, ReplMode::Loose);
    assert!(env.is_err());
}

#[test]
fn test_havoc_rem_euclid_zero_section() {
    let source = "a = seq_sections(section(bd, 0.00000000000000001))";
    let env = eval_module(source, ReplMode::Loose);
    assert!(env.is_err());
}
