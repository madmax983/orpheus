use orpheus_lang::{ReplMode, eval_module};

#[test]
fn test_stack_mixed_types() {
    let result = eval_module("x = stack(bd, 1)", ReplMode::Strict);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "`stack` requires all layers to be the same pattern kind"
    );
}

#[test]
fn test_eval_meter_large_float() {
    let source = "notes = meter(1e10, 4, at(beat(0), bd))";
    let res = eval_module(source, ReplMode::Loose);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.to_string()
            .contains("meter beat count must be a positive integer")
            || err.to_string().contains("parse error")
    );
}

#[test]
fn test_pedal_assign_unreachable() {
    let source = "notes = { x = 1; x = 2; x }";
    let res = eval_module(source, ReplMode::Loose);
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        err.to_string().contains("parse error")
            || err.to_string().contains("not supported")
            || err.to_string().contains("unreachable")
    );
}
