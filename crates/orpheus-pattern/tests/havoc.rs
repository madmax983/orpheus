use orpheus_pattern::{CyclePattern, Event, EventStream, Pattern, PatternNode, Rational, TimeSpan};
use std::panic;

#[test]
fn event_stream_query_does_not_panic() {
    let result = panic::catch_unwind(|| {
        let stream = EventStream::new(vec![Event {
            whole: None,
            part: TimeSpan::unit(),
            value: 42.0,
        }]);

        let query_span = TimeSpan::new(
            Rational::checked_from_parts(i64::MIN as i128, 1).unwrap(),
            Rational::checked_from_parts(i64::MAX as i128, 1).unwrap(),
        )
        .unwrap();

        let _ = stream.query(query_span);
    });

    assert!(result.is_ok(), "EventStream::query panicked");
}

#[test]
fn cycle_pattern_query_does_not_panic_on_extreme_limits() {
    let result = panic::catch_unwind(|| {
        let cycle = CyclePattern::from_nodes(vec![PatternNode::atom(42.0)]);

        let query_span = TimeSpan::new(
            Rational::checked_from_parts(i64::MIN as i128, 1).unwrap(),
            Rational::checked_from_parts(i64::MAX as i128, 1).unwrap(),
        )
        .unwrap();

        let _ = cycle.query(query_span);
    });

    assert!(result.is_ok(), "CyclePattern::query panicked on i64 bounds");
}

#[test]
fn cycle_pattern_query_does_not_panic_on_high_allocation_limits() {
    let result = panic::catch_unwind(|| {
        let cycle = CyclePattern::from_nodes(vec![PatternNode::atom(42.0)]);

        let query_span = TimeSpan::new(
            Rational::zero(),
            Rational::checked_from_parts(2_000_000, 1).unwrap(),
        )
        .unwrap();

        let _ = cycle.query(query_span);
    });

    assert!(
        result.is_ok(),
        "CyclePattern::query panicked on high allocation limit"
    );
}
