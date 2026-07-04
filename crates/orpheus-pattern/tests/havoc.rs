use orpheus_pattern::{CyclePattern, EventStream, Pattern, PatternNode, Rational, TimeSpan};
use proptest::prelude::*;

#[test]
fn havoc_query_no_panics_on_cycle_pattern() {
    let start = Rational::checked_from_parts(i128::MAX - 2, 1).unwrap();
    let end = Rational::checked_from_parts(i128::MAX - 1, 1).unwrap();
    let span = TimeSpan::new(start, end).unwrap();

    let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("a"), PatternNode::atom("b")]);

    let result = std::panic::catch_unwind(|| pattern.query(span));
    assert!(result.is_ok(), "query panicked");
    assert_eq!(result.unwrap().len(), 0);
}

#[test]
fn havoc_query_no_panics_on_event_stream() {
    let start = Rational::checked_from_parts(i128::MAX - 2, 1).unwrap();
    let end = Rational::checked_from_parts(i128::MAX - 1, 1).unwrap();
    let span = TimeSpan::new(start, end).unwrap();
    let stream = EventStream::<&str>::new(vec![]);
    let result = std::panic::catch_unwind(|| stream.query(span));
    assert!(result.is_ok(), "query panicked");
    assert_eq!(result.unwrap().len(), 0);
}

proptest! {
    #[test]
    fn havoc_rational_addition_panic(
        n1 in any::<i64>(),
        d1 in any::<i64>().prop_filter("non-zero", |&d| d != 0),
        n2 in any::<i64>(),
        d2 in any::<i64>().prop_filter("non-zero", |&d| d != 0)
    ) {
        if let (Ok(r1), Ok(r2)) = (Rational::checked_from_parts(i128::from(n1) * 100000000, i128::from(d1)), Rational::checked_from_parts(i128::from(n2) * 100000000, i128::from(d2))) {
            let _ = r1 + r2;
        }
    }
}
