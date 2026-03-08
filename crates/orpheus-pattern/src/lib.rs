//! Temporal pattern engine for Orpheus.

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimeSpan;

impl TimeSpan {
    #[must_use]
    pub const fn unit() -> Self {
        Self
    }

    #[must_use]
    pub const fn start_numer(&self) -> i64 {
        0
    }
}
