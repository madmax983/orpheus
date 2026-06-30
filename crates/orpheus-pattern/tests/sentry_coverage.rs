use orpheus_pattern::{CyclePattern, PatternError, PatternNode, Rational, TimeSpan};

#[test]
fn rational_multiplication_overflow() {
    let large = Rational::checked_from_parts(i128::MAX, 2).unwrap();
    let r = large.checked_mul(&large);
    assert_eq!(
        r,
        Err(PatternError::ArithmeticOverflow {
            operation: "rational multiplication"
        })
    );
}

#[test]
#[should_panic(expected = "rational addition overflowed during checked arithmetic")]
fn rational_add_panic_on_overflow() {
    let r1 = Rational::checked_from_parts(i128::MAX, 1).unwrap();
    let r2 = Rational::checked_from_parts(1, 1).unwrap();
    let _ = r1 + r2;
}

#[test]
fn cycle_pattern_huge_cycle_count() {
    let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("a")]);
    let start = Rational::zero();
    let end = Rational::checked_from_parts(200_000, 1).unwrap();
    let span = TimeSpan::new(start, end).unwrap();
    let res = pattern.try_query(&span);
    assert_eq!(
        res,
        Err(PatternError::ArithmeticOverflow {
            operation: "evaluation exceeded the maximum allowed event limit"
        })
    );
}

#[test]
fn cycle_pattern_huge_cycle_count_overflow() {
    let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("a")]);
    let start = Rational::checked_from_parts(-i128::MAX, 1).unwrap();
    let end = Rational::checked_from_parts(i128::MAX, 1).unwrap();
    let span = TimeSpan::new(start, end).unwrap();
    let res = pattern.try_query(&span);
    assert_eq!(
        res,
        Err(PatternError::ArithmeticOverflow {
            operation: "cycle count exceeded evaluator limits"
        })
    );
}

#[test]
fn rational_signed_from_magnitude_negation_overflow_trigger() {
    let res = Rational::checked_from_parts(i128::MIN, -1);
    assert_eq!(
        res,
        Err(PatternError::ArithmeticOverflow {
            operation: "rational normalization"
        })
    );
}

#[test]
fn rational_multiplication_overflow_denominator() {
    let large1 = Rational::checked_from_parts(1, i128::MAX).unwrap();
    let large2 = Rational::checked_from_parts(1, 2).unwrap();
    let r = large1.checked_mul(&large2);
    assert_eq!(
        r,
        Err(PatternError::ArithmeticOverflow {
            operation: "rational multiplication"
        })
    );
}
