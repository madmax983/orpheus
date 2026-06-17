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

proptest! {
    #[test]
    fn havoc_rational_checked_normalize_bounds(num in proptest::num::i128::ANY, den in proptest::num::i128::ANY) {
        // Run massive fuzzing over i128 range to make sure Rational initialization
        // correctly catches and bubbles `ArithmeticOverflow` or `InvalidDenominator`
        // without panicking on native boundaries.
        let result = std::panic::catch_unwind(|| {
            let _ = Rational::checked_from_parts(num, den);
        });

        assert!(result.is_ok(), "Rational arithmetic panics on {num}/{den} instead of returning PatternError");
    }
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
