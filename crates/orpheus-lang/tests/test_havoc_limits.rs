use orpheus_lang::{ReplMode, eval_module};
use proptest::prelude::*;
use std::panic;

#[test]
fn test_havoc_division_by_zero_speed() {
    let source = "res = slow(0, bd)";
    let result = panic::catch_unwind(|| {
        let _ = eval_module(source, ReplMode::Loose);
    });
    assert!(result.is_ok());
}

#[test]
fn test_havoc_every_zero() {
    let source = "res = every(0, rev, bd)";
    let result = panic::catch_unwind(|| {
        let _ = eval_module(source, ReplMode::Loose);
    });
    assert!(result.is_ok());
}

#[test]
fn test_havoc_chop_inf() {
    let source = "res = chop(inf, bd)";
    let result = panic::catch_unwind(|| {
        let _ = eval_module(source, ReplMode::Loose);
    });
    assert!(result.is_ok());
}

#[test]
fn test_havoc_nested_curry_eval_error_query() {
    let source = "f x y = fast(2, x y)";
    let result = panic::catch_unwind(|| {
        let _ = eval_module(source, ReplMode::Loose);
    });
    assert!(result.is_ok());
}
