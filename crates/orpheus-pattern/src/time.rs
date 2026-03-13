//! Continuous time representation for patterns.
//!
//! Orpheus uses half-open time intervals to represent when events occur and how
//! long they last. All time is continuous and represented exactly using rational
//! numbers.

use core::cmp::Ordering;

use crate::{PatternError, Rational};

/// Half-open time span in exact rational time: `[start, end)`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TimeSpan {
    start: Rational,
    end: Rational,
}

impl TimeSpan {
    /// Creates a half-open span if and only if its bounds are ordered.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_pattern::{Rational, TimeSpan};
    ///
    /// let start = Rational::new(1, 4).unwrap();
    /// let end = Rational::new(3, 4).unwrap();
    /// let span = TimeSpan::new(start, end).unwrap();
    ///
    /// assert_eq!(span.start_numer(), 1);
    /// ```
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

    /// Returns the unit cycle span `[0, 1)`.
    #[must_use]
    pub const fn unit() -> Self {
        Self {
            start: Rational::zero(),
            end: Rational::one(),
        }
    }

    /// Returns the inclusive start bound of the span.
    #[must_use]
    pub const fn start(&self) -> &Rational {
        &self.start
    }

    /// Returns the exclusive end bound of the span.
    #[must_use]
    pub const fn end(&self) -> &Rational {
        &self.end
    }

    /// Returns `true` when the half-open span contains no duration.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start() == self.end()
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
