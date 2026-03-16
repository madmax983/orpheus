use orpheus_pattern::{PatternError, Rational, TimeSpan};

#[test]
fn rational_thirds_sum_exactly_to_one() {
    let third = Rational::new(1, 3).unwrap();
    assert_eq!(third.clone() + third.clone() + third, Rational::one());
}

#[test]
fn timespan_new_rejects_end_before_start() {
    let start = Rational::new(2, 1).unwrap();
    let end = Rational::new(1, 1).unwrap();
    assert!(TimeSpan::new(start, end).is_err());
}

#[test]
fn rational_new_rejects_zero_denominator() {
    assert_eq!(
        Rational::new(1, 0),
        Err(PatternError::InvalidDenominator { denominator: 0 })
    );
}

#[test]
fn rational_new_normalizes_sign_and_reduces() {
    let reduced = Rational::new(2, -4).unwrap();

    assert_eq!(reduced, Rational::new(-1, 2).unwrap());
    assert_eq!(reduced.numerator(), -1);
    assert_eq!(reduced.denominator(), 2);
}

#[test]
fn timespan_new_accepts_zero_length_spans() {
    let point = Rational::new(3, 4).unwrap();
    let span = TimeSpan::new(point.clone(), point).unwrap();

    assert_eq!(span.start(), span.end());
}

#[test]
fn rational_checked_add_reports_overflow() {
    let first = Rational::new(1, (1_i64 << 41) - 1).unwrap();
    let second = Rational::new(1, (1_i64 << 43) - 1).unwrap();
    let third = Rational::new(1, (1_i64 << 47) - 1).unwrap();
    let partial = first.checked_add(&second).unwrap();

    assert_eq!(
        partial.checked_add(&third),
        Err(PatternError::ArithmeticOverflow {
            operation: "rational addition",
        })
    );
}

#[test]
fn timespan_default_returns_unit_span() {
    let span = TimeSpan::default();
    assert_eq!(span, TimeSpan::unit());
}
