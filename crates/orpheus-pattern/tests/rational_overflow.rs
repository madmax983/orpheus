use orpheus_pattern::Rational;

#[test]
#[should_panic(expected = "rational addition overflowed during checked arithmetic")]
fn test_rational_addition_overflow_panics() {
    let a = Rational::checked_from_parts(i128::MAX, 1).unwrap();
    let b = Rational::checked_from_parts(1, 1).unwrap();
    let _ = a + b;
}

#[test]
fn test_rational_mul_overflow_reports_error() {
    let a = Rational::checked_from_parts(i128::MAX, 1).unwrap();
    let b = Rational::checked_from_parts(2, 1).unwrap();
    let res = a.checked_mul(&b);
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().to_string(),
        "rational multiplication exceeded the supported range"
    );
}

#[test]
fn test_rational_sub_overflow_reports_error() {
    // sub uses addition, so we use MIN/MAX
    let a = Rational::checked_from_parts(i128::MIN, 1).unwrap();
    let b = Rational::checked_from_parts(1, 1).unwrap();
    let res = a.checked_sub(&b);
    assert!(res.is_err());
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("exceeded the supported range")
    );
}
