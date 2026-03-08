//! Temporal pattern engine for Orpheus.

/// Minimal cycle-relative span placeholder used to link the workspace.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TimeSpan;

impl TimeSpan {
    /// Returns the unit span used by the bootstrap smoke test.
    #[must_use]
    pub const fn unit() -> Self {
        Self
    }

    /// Returns the start numerator for the placeholder unit span.
    #[must_use]
    pub const fn start_numer(&self) -> i64 {
        0
    }
}
