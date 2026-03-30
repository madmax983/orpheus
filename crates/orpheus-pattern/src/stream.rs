//! Finite event streams in explicit time.
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

        let mut events = Vec::new();
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
        self.try_query(&span).unwrap_or_default()
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
