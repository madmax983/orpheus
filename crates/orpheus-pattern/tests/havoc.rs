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
    fn test_havoc_rational_operations_do_not_panic(
        n1 in proptest::num::i128::ANY,
        d1 in proptest::num::i128::ANY,
        n2 in proptest::num::i128::ANY,
        d2 in proptest::num::i128::ANY
    ) {
        if let (Ok(r1), Ok(r2)) = (Rational::checked_from_parts(n1, d1), Rational::checked_from_parts(n2, d2)) {
            let _ = r1.checked_add(&r2);
            let _ = r1.checked_sub(&r2);
            let _ = r1.checked_mul(&r2);
            let _ = r1.checked_cmp(&r2);
        }
    }

    #[test]
    fn test_havoc_rational_add_trait_panic(
        n1 in proptest::num::i64::ANY,
        d1 in proptest::num::i64::ANY,
        n2 in proptest::num::i64::ANY,
        d2 in proptest::num::i64::ANY
    ) {
        if let (Ok(r1), Ok(r2)) = (Rational::new(n1, d1), Rational::new(n2, d2)) {
            let _ = r1 + r2;
        }
    }
}
