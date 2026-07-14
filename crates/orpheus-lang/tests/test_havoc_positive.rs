//! Tests evaluating edge cases with extremely small positive numbers and float meters to ensure precise timing precision.
use orpheus_lang::{ReplMode, eval_module};

#[test]
fn test_havoc_tiny_float_meter_panic() {
    let source = "notes = meter(0.00000000000000001, 4, at(beat(0), bd))";
    let res = eval_module(source, ReplMode::Loose);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(
        err.to_string(),
        "meter beat count must be a positive integer"
    );
}

#[test]
fn test_havoc_negative_float_meter_panic() {
    let source = "notes = meter(-4, 4, at(beat(0), bd))";
    let res = eval_module(source, ReplMode::Loose);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert_eq!(
        err.to_string(),
        "meter beat count must be a positive integer"
    );
}

// Well wait, I found a piece of code that is unreachable!
// `value.is_nan()` right after `!value.is_finite() || ...`
