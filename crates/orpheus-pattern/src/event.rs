use crate::TimeSpan;

/// Value scheduled over an exact time span.
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
