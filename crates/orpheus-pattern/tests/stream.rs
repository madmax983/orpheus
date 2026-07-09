//! Integration tests for the `EventStream` composition and consumption behavior, checking how events are interleaved or blocked over continuous sequences.
use orpheus_pattern::{Event, EventStream, Pattern, Rational, TimeSpan};

fn rational(numerator: i64, denominator: i64) -> Rational {
    Rational::new(numerator, denominator).unwrap()
}

fn span(
    start_numerator: i64,
    start_denominator: i64,
    end_numerator: i64,
    end_denominator: i64,
) -> TimeSpan {
    TimeSpan::new(
        rational(start_numerator, start_denominator),
        rational(end_numerator, end_denominator),
    )
    .unwrap()
}

#[test]
fn stream_query_returns_events_in_explicit_time_order() {
    let stream = EventStream::new(vec![
        Event {
            whole: None,
            part: span(5, 2, 3, 1),
            value: "late",
        },
        Event {
            whole: None,
            part: span(0, 1, 1, 1),
            value: "early",
        },
    ]);

    let events = stream.query(span(0, 1, 4, 1));

    assert_eq!(events.len(), 2);
    assert_eq!(events[0].value, "early");
    assert_eq!(events[1].value, "late");
}

#[test]
fn stream_query_handles_empty_query_window() {
    let stream = EventStream::new(vec![Event {
        whole: None,
        part: span(0, 1, 1, 1),
        value: "event",
    }]);

    let events = stream.query(span(2, 1, 2, 1));
    assert_eq!(events.len(), 0);
}

#[test]
fn stream_query_clips_partial_events() {
    let stream = EventStream::new(vec![Event {
        whole: None,
        part: span(0, 1, 2, 1),
        value: "long_event",
    }]);

    let events = stream.query(span(1, 1, 3, 1));
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].part, span(1, 1, 2, 1));
    assert_eq!(events[0].whole, Some(span(0, 1, 2, 1)));
}

#[test]
fn stream_query_ignores_out_of_bounds_events() {
    let stream = EventStream::new(vec![
        Event {
            whole: None,
            part: span(0, 1, 1, 1),
            value: "early",
        },
        Event {
            whole: None,
            part: span(3, 1, 4, 1),
            value: "late",
        },
    ]);

    let events = stream.query(span(1, 1, 2, 1));
    assert_eq!(events.len(), 0);
}

#[test]
fn stream_try_query_handles_empty_query_window() {
    let stream = EventStream::new(vec![Event {
        whole: None,
        part: span(0, 1, 1, 1),
        value: "event",
    }]);

    let events = stream.try_query(&span(2, 1, 2, 1)).unwrap();
    assert_eq!(events.len(), 0);
}

#[test]
fn stream_try_query_replaces_whole_when_fully_contained() {
    let stream = EventStream::new(vec![Event {
        whole: Some(span(1, 2, 3, 2)),
        part: span(1, 2, 3, 2),
        value: "event",
    }]);

    let events = stream.try_query(&span(0, 1, 2, 1)).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].whole, None);
    assert_eq!(events[0].part, span(1, 2, 3, 2));
}

#[test]
fn stream_try_query_preserves_whole_when_clipped() {
    let stream = EventStream::new(vec![Event {
        whole: None,
        part: span(1, 2, 3, 2),
        value: "event",
    }]);

    let events = stream.try_query(&span(1, 1, 2, 1)).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].whole, Some(span(1, 2, 3, 2)));
    assert_eq!(events[0].part, span(1, 1, 3, 2));
}

#[test]
fn stream_try_query_clips_partial_events_with_correct_bounds() {
    let stream = EventStream::new(vec![Event {
        whole: None,
        part: span(1, 4, 3, 4),
        value: "mid",
    }]);

    let events = stream.try_query(&span(1, 2, 1, 1)).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].part, span(1, 2, 3, 4));
}

#[test]
fn stream_query_returns_empty_when_span_has_zero_duration() {
    let stream = EventStream::new(vec![Event {
        whole: None,
        part: span(0, 1, 1, 1),
        value: "event",
    }]);

    let events = stream.query(span(1, 2, 1, 2));
    assert_eq!(events.len(), 0);
}
