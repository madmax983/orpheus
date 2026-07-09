//! Chaos/Fuzz testing for the pattern matching logic, aggressively probing extreme bounding, large divisions, and infinite cycle edge cases to ensure numeric safety.
use orpheus_pattern::{CyclePattern, EventStream, Pattern, PatternNode, Rational, TimeSpan};

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
