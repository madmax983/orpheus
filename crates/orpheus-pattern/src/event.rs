use crate::TimeSpan;

/// Value scheduled over an exact time span.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Event<T> {
    pub span: TimeSpan,
    pub value: T,
}
