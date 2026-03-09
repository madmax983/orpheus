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
