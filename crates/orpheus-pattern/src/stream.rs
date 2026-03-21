//! The `stream` module implements finite event streams in explicit time.
//!
//! Unlike repeating patterns, an [`EventStream`] has a definite end and represents
//! a fixed sequence of scheduled events.

use core::cmp::{max, min};

use crate::{Event, Pattern, PatternError, TimeSpan};

/// A finite explicit-time event stream.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventStream<T> {
    events: Vec<Event<T>>,
}

impl<T> EventStream<T> {
    /// Creates an event stream from explicit-time events.
    ///
    /// The events are sorted by their start time when the stream is created.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_pattern::{Event, EventStream, Rational, TimeSpan};
    ///
    /// let event = Event {
    ///     whole: None,
    ///     part: TimeSpan::unit(),
    ///     value: "bd",
    /// };
    ///
    /// let stream = EventStream::new(vec![event]);
    /// ```
    #[must_use]
    pub fn new(mut events: Vec<Event<T>>) -> Self {
        events.sort_by(|left, right| {
            left.part
                .start()
                .cmp(right.part.start())
                .then(left.part.end().cmp(right.part.end()))
        });
        Self { events }
    }

    /// Queries the stream over the half-open window `span`.
    ///
    /// # Errors
    ///
    /// Returns any span-construction error encountered while clipping stored
    /// events to the query window.
    ///
    /// ## Examples
    ///
    /// ```
    /// use orpheus_pattern::{EventStream, Event, TimeSpan, Rational};
    ///
    /// let event = Event {
    ///     value: 42,
    ///     whole: None,
    ///     part: TimeSpan::unit(),
    /// };
    /// let stream = EventStream::new(vec![event]);
    ///
    /// let result = stream.try_query(&TimeSpan::unit()).unwrap();
    /// assert_eq!(result.len(), 1);
    /// assert_eq!(result[0].value, 42);
    /// ```
    pub fn try_query(&self, span: &TimeSpan) -> Result<Vec<Event<T>>, PatternError>
    where
        T: Clone,
    {
        if span.is_empty() {
            return Ok(Vec::new());
        }

        // ⚡ Bolt: Pre-allocate vector to the maximum possible size to prevent heap reallocations during stream querying.
        let mut events = Vec::with_capacity(self.events.len());
        for event in &self.events {
            let whole = event.whole.as_ref().unwrap_or(&event.part);
            if let Some(part) = clip_span(whole, span)? {
                events.push(Event {
                    whole: if part == *whole {
                        None
                    } else {
                        Some(whole.clone())
                    },
                    part,
                    value: event.value.clone(),
                });
            }
        }

        Ok(events)
    }
}

impl<T> Pattern<T> for EventStream<T>
where
    T: Clone + Send + Sync,
{
    fn query(&self, span: TimeSpan) -> Vec<Event<T>> {
        self.try_query(&span)
            .unwrap_or_else(|error| panic!("event stream query failed for span {span:?}: {error}"))
    }
}

fn clip_span(span: &TimeSpan, query: &TimeSpan) -> Result<Option<TimeSpan>, PatternError> {
    let start = max(span.start(), query.start()).clone();
    let end = min(span.end(), query.end()).clone();

    if start >= end {
        return Ok(None);
    }

    TimeSpan::new(start, end).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rational;

    #[test]
    fn new_sorts_events_by_start_time_then_end_time() {
        let e1 = Event {
            whole: None,
            part: TimeSpan::new(Rational::new(1, 4).unwrap(), Rational::new(1, 2).unwrap())
                .unwrap(),
            value: 1,
        };
        let e2 = Event {
            whole: None,
            part: TimeSpan::new(Rational::new(1, 4).unwrap(), Rational::new(3, 4).unwrap())
                .unwrap(),
            value: 2,
        };
        let e3 = Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
            value: 3,
        };

        // e3 starts at 0, e1 and e2 start at 1/4. e1 ends at 1/2, e2 ends at 3/4.
        // Expected order: e3, e1, e2.
        let stream = EventStream::new(vec![e2, e1, e3]);

        assert_eq!(stream.events[0].value, 3);
        assert_eq!(stream.events[1].value, 1);
        assert_eq!(stream.events[2].value, 2);
    }

    #[test]
    fn try_query_returns_empty_when_span_is_empty() {
        let event = Event {
            whole: None,
            part: TimeSpan::unit(),
            value: 42,
        };
        let stream = EventStream::new(vec![event]);

        let empty_span = TimeSpan::new(Rational::zero(), Rational::zero()).unwrap();
        let result = stream.try_query(&empty_span).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn try_query_returns_empty_when_span_does_not_overlap() {
        let event = Event {
            whole: None,
            part: TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap(),
            value: 42,
        };
        let stream = EventStream::new(vec![event]);

        let query_span = TimeSpan::new(Rational::new(1, 2).unwrap(), Rational::one()).unwrap();
        let result = stream.try_query(&query_span).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn try_query_returns_unclipped_event_when_span_fully_contains_it() {
        let event = Event {
            whole: None,
            part: TimeSpan::new(Rational::new(1, 4).unwrap(), Rational::new(1, 2).unwrap())
                .unwrap(),
            value: 42,
        };
        let stream = EventStream::new(vec![event.clone()]);

        let query_span = TimeSpan::unit();
        let result = stream.try_query(&query_span).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], event);
    }

    #[test]
    fn try_query_clips_event_when_span_partially_overlaps() {
        let event_span =
            TimeSpan::new(Rational::new(1, 4).unwrap(), Rational::new(3, 4).unwrap()).unwrap();
        let event = Event {
            whole: None,
            part: event_span.clone(),
            value: 42,
        };
        let stream = EventStream::new(vec![event]);

        // Query overlaps the first half of the event
        let query_span = TimeSpan::new(Rational::zero(), Rational::new(1, 2).unwrap()).unwrap();
        let result = stream.try_query(&query_span).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].value, 42);
        assert_eq!(result[0].whole, Some(event_span));

        let expected_part =
            TimeSpan::new(Rational::new(1, 4).unwrap(), Rational::new(1, 2).unwrap()).unwrap();
        assert_eq!(result[0].part, expected_part);
    }

    #[test]
    fn query_unwraps_try_query_result() {
        let event = Event {
            whole: None,
            part: TimeSpan::unit(),
            value: "test",
        };
        let stream = EventStream::new(vec![event.clone()]);

        let result = stream.query(TimeSpan::unit());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], event);
    }

    #[test]
    fn clip_span_returns_none_when_non_overlapping() {
        let span1 = TimeSpan::new(Rational::zero(), Rational::new(1, 4).unwrap()).unwrap();
        let span2 = TimeSpan::new(Rational::new(1, 2).unwrap(), Rational::one()).unwrap();

        let result = clip_span(&span1, &span2).unwrap();
        assert_eq!(result, None);

        // Adjacent spans also do not overlap because they are half-open [start, end)
        let span3 =
            TimeSpan::new(Rational::new(1, 4).unwrap(), Rational::new(1, 2).unwrap()).unwrap();
        let result_adjacent = clip_span(&span1, &span3).unwrap();
        assert_eq!(result_adjacent, None);
    }

    #[test]
    fn clip_span_returns_intersection_when_overlapping() {
        let span1 = TimeSpan::new(Rational::zero(), Rational::new(3, 4).unwrap()).unwrap();
        let span2 = TimeSpan::new(Rational::new(1, 4).unwrap(), Rational::one()).unwrap();

        let result = clip_span(&span1, &span2).unwrap();
        let expected =
            TimeSpan::new(Rational::new(1, 4).unwrap(), Rational::new(3, 4).unwrap()).unwrap();
        assert_eq!(result, Some(expected));
    }
}
