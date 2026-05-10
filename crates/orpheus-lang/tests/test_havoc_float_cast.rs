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
