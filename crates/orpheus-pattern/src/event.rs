//! Events scheduled in exact time.
//!
//! An event represents a value that is conceptually active over a given span of
//! time.

use crate::TimeSpan;

/// Value scheduled over an exact time span.
///
/// When an event is returned by a pattern query window, it may be clipped. The
/// `part` field shows the portion of the event that falls inside the queried
/// window. If the event was clipped, the `whole` field holds the original unclipped
/// duration.
///
/// # Examples
///
/// ```
/// use orpheus_pattern::{Event, Rational, TimeSpan};
///
/// let whole_span = TimeSpan::unit();
/// let part_span = TimeSpan::new(
///     Rational::new(1, 2).unwrap(),
///     Rational::one(),
/// ).unwrap();
///
/// // Represents an event that originally lasted the whole cycle,
/// // but was clipped to just the second half by the query window.
/// let event = Event {
///     whole: Some(whole_span),
///     part: part_span,
///     value: "bd",
/// };
/// ```
///
/// Or an unclipped event:
/// ```
/// use orpheus_pattern::{Event, TimeSpan};
///
/// // Represents an event that lasted exactly the query window.
/// let unclipped_event = Event {
///     whole: None,
///     part: TimeSpan::unit(),
///     value: "sn",
/// };
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event<T> {
    /// The full span the event conceptually occupies.
    ///
    /// This is `Some(...)` only when the event was clipped to the query window.
    pub whole: Option<TimeSpan>,
    /// The portion of the event that falls within the query window.
    pub part: TimeSpan,
    /// The payload carried by the event.
    pub value: T,
}
