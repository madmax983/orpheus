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
fn test_havoc_large_section_cycle_count_evaluator_limits() {
    let source = "notes = seq_sections(section(bd, 1025))";
    let res = eval_module(source, ReplMode::Loose);
    let err = res.unwrap_err();
    assert_eq!(
        err.to_string(),
        "section cycle count exceeded the maximum allowed bound of 1024"
    );
}
