use core::cmp::Ordering;

use crate::{PatternError, Rational};

/// Closed time span in exact rational time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeSpan {
    start: Rational,
    end: Rational,
}

impl TimeSpan {
    /// Creates a span if and only if its bounds are ordered.
    ///
    /// # Errors
    ///
    /// Returns [`PatternError::InvalidSpan`] if `start` is after `end`.
    pub fn new(start: Rational, end: Rational) -> Result<Self, PatternError> {
        if matches!(start.checked_cmp(&end)?, Ordering::Greater) {
            return Err(PatternError::InvalidSpan { start, end });
        }

        Ok(Self { start, end })
    }

    /// Returns the unit cycle span.
    #[must_use]
    pub const fn unit() -> Self {
        Self {
            start: Rational::zero(),
            end: Rational::one(),
        }
    }

    /// Returns the start bound of the span.
    #[must_use]
    pub const fn start(&self) -> &Rational {
        &self.start
    }

    /// Returns the end bound of the span.
    #[must_use]
    pub const fn end(&self) -> &Rational {
        &self.end
    }

    /// Returns the normalized numerator of the span start.
    #[must_use]
    pub const fn start_numer(&self) -> i128 {
        self.start().numerator()
    }
}

impl Default for TimeSpan {
    fn default() -> Self {
        Self::unit()
    }
}
