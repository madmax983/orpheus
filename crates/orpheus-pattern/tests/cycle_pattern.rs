use orpheus_pattern::{CyclePattern, Pattern, PatternNode, Rational, TimeSpan};
use proptest::prelude::*;

#[test]
fn cycle_pattern_divides_unit_span_evenly() {
    let pattern = CyclePattern::from_nodes(vec![
        PatternNode::atom("bd"),
        PatternNode::atom("sn"),
        PatternNode::atom("cp"),
    ]);

    let events = pattern.query(TimeSpan::unit());

    assert_eq!(events.len(), 3);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[1].part.start(), &Rational::new(1, 3).unwrap());
    assert_eq!(events[2].part.start(), &Rational::new(2, 3).unwrap());
    assert!(events.iter().all(|event| event.whole.is_none()));
}

#[test]
fn cycle_pattern_subdivides_nested_groups() {
    let pattern = CyclePattern::from_nodes(vec![
        PatternNode::group(vec![PatternNode::atom("bd"), PatternNode::atom("sn")]),
        PatternNode::atom("cp"),
    ]);

    let events = pattern.query(TimeSpan::unit());

    assert_eq!(events.len(), 3);
    assert_eq!(events[0].part.start(), &Rational::zero());
    assert_eq!(events[0].part.end(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[1].part.start(), &Rational::new(1, 4).unwrap());
    assert_eq!(events[1].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[2].part.end(), &Rational::one());
}

#[test]
fn cycle_pattern_repeats_across_cycle_boundaries() {
    let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("bd")]);
    let span = TimeSpan::new(Rational::one(), Rational::new(2, 1).unwrap()).unwrap();

    let events = pattern.query(span);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].part.start(), &Rational::one());
    assert_eq!(events[0].part.end(), &Rational::new(2, 1).unwrap());
    assert!(events[0].whole.is_none());
}

#[test]
fn cycle_pattern_clips_whole_spans_to_query_window() {
    let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("bd"), PatternNode::atom("sn")]);
    let span = TimeSpan::new(Rational::new(1, 4).unwrap(), Rational::new(3, 4).unwrap()).unwrap();

    let events = pattern.query(span);

    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0].whole.as_ref().unwrap(),
        &TimeSpan::new(Rational::zero(), Rational::new(1, 2).unwrap()).unwrap()
    );
    assert_eq!(events[0].part.start(), span.start());
    assert_eq!(events[0].part.end(), &Rational::new(1, 2).unwrap());
    assert_eq!(
        events[1].whole.as_ref().unwrap(),
        &TimeSpan::new(Rational::new(1, 2).unwrap(), Rational::one()).unwrap()
    );
    assert_eq!(events[1].part.start(), &Rational::new(1, 2).unwrap());
    assert_eq!(events[1].part.end(), span.end());
}

#[test]
fn cycle_pattern_clips_partial_windows_across_cycle_boundaries() {
    let pattern = CyclePattern::from_nodes(vec![PatternNode::atom("bd")]);
    let span = TimeSpan::new(Rational::new(3, 4).unwrap(), Rational::new(5, 4).unwrap()).unwrap();

    let events = pattern.query(span);

    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0].whole.as_ref().unwrap(),
        &TimeSpan::new(Rational::zero(), Rational::one()).unwrap()
    );
    assert_eq!(events[0].part.start(), &Rational::new(3, 4).unwrap());
    assert_eq!(events[0].part.end(), &Rational::one());
    assert_eq!(
        events[1].whole.as_ref().unwrap(),
        &TimeSpan::new(Rational::one(), Rational::new(2, 1).unwrap()).unwrap()
    );
    assert_eq!(events[1].part.start(), &Rational::one());
    assert_eq!(events[1].part.end(), &Rational::new(5, 4).unwrap());
}

proptest! {
    #[test]
    fn queried_event_parts_stay_within_requested_span(
        start_numer in -4_i64..=4,
        start_denom in 1_i64..=4,
        extra_numer in 1_i64..=8,
        extra_denom in 1_i64..=4,
    ) {
        let pattern = CyclePattern::from_nodes(vec![
            PatternNode::atom("bd"),
            PatternNode::group(vec![PatternNode::atom("sn"), PatternNode::rest()]),
            PatternNode::atom("cp"),
        ]);
        let start = Rational::new(start_numer, start_denom).unwrap();
        let width = Rational::new(extra_numer, extra_denom).unwrap();
        let end = start.checked_add(&width).unwrap();
        let span = TimeSpan::new(start, end).unwrap();

        for event in pattern.query(span) {
            prop_assert!(event.part.start() >= span.start());
            prop_assert!(event.part.end() <= span.end());
        }
    }
}
