use orpheus_pattern::{PatternError, Rational};

#[test]
fn rational_checked_mul_reports_overflow() {
    let huge = Rational::checked_from_parts(i128::MAX, 1).unwrap();
    let two = Rational::checked_from_parts(2, 1).unwrap();
    assert_eq!(huge.checked_mul(&two), Err(PatternError::ArithmeticOverflow { operation: "rational multiplication" }));
}

#[test]
fn rational_checked_sub_reports_overflow() {
    let huge_neg = Rational::checked_from_parts(i128::MIN + 2, 1).unwrap();
    let two = Rational::checked_from_parts(3, 1).unwrap();
    assert_eq!(huge_neg.checked_sub(&two), Err(PatternError::ArithmeticOverflow { operation: "rational addition" }));
}

#[test]
fn rational_checked_sub_reports_overflow_negate() {
    // trying to negate i128::MIN should trigger overflow when it's done during `checked_sub` because of `checked_normalize`
    let min = Rational::checked_from_parts(i128::MIN, 1).unwrap();
    let one = Rational::checked_from_parts(1, 1).unwrap();
    assert_eq!(one.checked_sub(&min), Err(PatternError::ArithmeticOverflow { operation: "rational normalization" }));
}

#[test]
fn rational_checked_add_reports_overflow_normalization() {
    let huge = Rational::checked_from_parts(i128::MAX, 1).unwrap();
    let huge2 = Rational::checked_from_parts(2, 1).unwrap();
    assert_eq!(huge.checked_add(&huge2), Err(PatternError::ArithmeticOverflow { operation: "rational addition" }));
}
