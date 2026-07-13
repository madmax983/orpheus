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
fn test_havoc_float_cast_precision_loss() {
    // 👺 Havoc: Tests that large float bounds bypass due to precision loss
    // safely saturates and generates an EvalError rather than silently wrapping
    // or panicking when casting `i128::MAX` equivalents.
    let source = "notes = meter(170141183460469231731687303715884105727.0, 4, at(beat(0), bd))";
    let res = eval_module(source, ReplMode::Loose);
    let err = res.unwrap_err();
    assert_eq!(
        err.to_string(),
        "meter beat count exceeded the supported range"
    );
}
