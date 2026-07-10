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
fn test_havoc_float_cast2() {
    // A value representing 1e38 which is below i128::MAX (1.7e38) so it passes f64_to_rational's numeric parsing
    // but should still gracefully fail within meter validation instead of panicking.
    let source = "notes = meter(100000000000000000000000000000000000000, 4, at(beat(0), bd))";
    let res = eval_module(source, ReplMode::Loose);
    let err = res.unwrap_err();
    assert_eq!(
        err.to_string(),
        "meter beat count exceeded the supported range"
    );
}

proptest::proptest! {
    #[test]
    fn test_havoc_f64_to_rational_prop(v in proptest::num::f64::ANY) {
        let _ = orpheus_lang::f64_to_rational(v, "test context");
    }
}
