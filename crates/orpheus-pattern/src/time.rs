//! The `time` module provides continuous time representation for patterns.
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

    /// The fundamental window of musical time, representing exactly one measure `[0, 1)`.
    ///
    /// In Orpheus, patterns are infinite functions of time. To render a pattern into discrete
    /// events, we query it over a specific window. The `unit()` span is the most common query
    /// window, asking the pattern to yield all events that occur during its first complete cycle.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_pattern::{Rational, TimeSpan};
    ///
    /// let span = TimeSpan::unit();
    /// assert_eq!(span.start(), &Rational::zero());
    /// assert_eq!(span.end(), &Rational::one());
    /// ```
    #[must_use]
    pub const fn unit() -> Self {
        Self {
            start: Rational::zero(),
            end: Rational::one(),
        }
    }

    /// The exact rational timeline point where this span begins (inclusive).
    ///
    /// When querying a pattern, this represents the start of the temporal window. When attached
    /// to an `Event`, it denotes the exact moment the sample or note should trigger.
    /// Because it returns a `Rational`, it avoids floating point jitter.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_pattern::{Rational, TimeSpan};
    ///
    /// let span = TimeSpan::unit();
    /// assert_eq!(span.start(), &Rational::zero());
    /// ```
    #[must_use]
    pub const fn start(&self) -> &Rational {
        &self.start
    }

    /// The exact rational timeline point where this span concludes (exclusive).
    ///
    /// Spans in Orpheus are half-open (`[start, end)`). This means an event whose start matches
    /// this end bound exactly will fall into the *next* adjacent span.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_pattern::{Rational, TimeSpan};
    ///
    /// let span = TimeSpan::unit();
    /// assert_eq!(span.end(), &Rational::one());
    /// ```
    #[must_use]
    pub const fn end(&self) -> &Rational {
        &self.end
    }

    /// Returns `true` when the half-open span contains no duration.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_pattern::{Rational, TimeSpan};
    ///
    /// let zero = Rational::zero();
    /// let span = TimeSpan::new(zero.clone(), zero).unwrap();
    /// assert!(span.is_empty());
    ///
    /// let unit = TimeSpan::unit();
    /// assert!(!unit.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start() == self.end()
    }

    /// Extracts the unreduced numerator of the span's start time.
    ///
    /// This provides a quick scalar identifier often used by renderers (like ASCII or SVG grids)
    /// to determine block offsets when the grid resolution exactly matches the span denominator.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_pattern::{Rational, TimeSpan};
    ///
    /// let start = Rational::new(3, 4).unwrap();
    /// let end = Rational::one();
    /// let span = TimeSpan::new(start, end).unwrap();
    /// assert_eq!(span.start_numer(), 3);
    /// ```
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
