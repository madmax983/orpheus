use orpheus_pattern::{Rational, TimeSpan};

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
