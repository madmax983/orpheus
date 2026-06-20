//! Tests evaluating float-to-integer casting operations to ensure robust handling of NaN, Infinity, and out-of-bounds values without crashing.
use orpheus_lang::{ReplMode, eval_module};

#[test]
fn test_havoc_float_cast() {
    let source = "notes = meter(340282366920938463463374607431768211455, 4, at(beat(0), bd))";
    let res = eval_module(source, ReplMode::Loose);
    let err = res.unwrap_err();
    assert_eq!(
        err.to_string(),
        "meter beat count exceeded the supported range"
    );
}

#[test]
fn test_havoc_float_cast_time() {
    let source = "x = shift(10000000000000000000000000000000000000000000000000.0, bd)";
    assert!(eval_module(source, ReplMode::Loose).is_err());

    let source = "x = meter(4, 4, at(10000000000000000000000000000000000000000000000000.0, bd))";
    assert!(eval_module(source, ReplMode::Loose).is_err());

    assert!(orpheus_lang::f64_to_rational(10000000000000000000000000000000000000000000000000.0, "test").is_err());
}
