//! Temporal pattern engine for Orpheus.
//!
//! This crate provides the data structures and semantics for scheduling events
//! in time. Time in Orpheus is continuous and represented exactly using rational
//! numbers.
//!
//! There are two main concepts of time provided here:
//! - **Cycle Patterns:** Infinitely repeating structures (like `CyclePattern`)
//!   that map elements to fractions of a unit cycle.
//! - **Explicit Streams:** Finite sequences of events (like `EventStream`) that
//!   have a definite start and end.
//!
//! Both of these implement the [`Pattern`] trait, which allows querying them
//! over a given half-open window of time called a [`TimeSpan`].

mod cycle;
mod event;
mod rational;
mod stream;
mod time;

pub use cycle::{CyclePattern, Pattern, PatternNode};
pub use event::Event;
pub use rational::Rational;
pub use stream::EventStream;
pub use time::TimeSpan;

use thiserror::Error;

/// Errors produced by the pattern core time model.
///
/// These errors occur when the internal mathematical invariants of Orpheus's continuous
/// time system are violated. Time in Orpheus is absolute, exact, and represented by rational
/// numbers; therefore, operations that would result in undefined math (like dividing by zero
/// or creating impossible spans of time) are trapped here.
///
/// ## Examples
///
/// Pattern errors are typically encountered when constructing invalid time values manually,
/// and they can be matched to provide helpful feedback to users constructing custom patterns.
///
/// ```
/// use orpheus_pattern::{PatternError, Rational, TimeSpan};
///
/// // Attempting to create a time span that flows backward in time:
/// let start = Rational::new(2, 1).unwrap();
/// let end = Rational::one();
///
/// let result = TimeSpan::new(start, end);
///
/// assert!(matches!(
///     result,
///     Err(PatternError::InvalidSpan { .. })
/// ));
/// ```

#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum PatternError {
    /// A rational value was constructed with a zero denominator.
    ///
    /// Orpheus uses exact rational fractions for time. If a denominator of zero is introduced,
    /// it implies a division by zero in the time domain, which is mathematically undefined.
    #[error("rational denominator cannot be zero")]
    InvalidDenominator {
        /// The literal zero value that was passed as the denominator.
        denominator: i64,
    },
    /// A checked rational operation exceeded the supported integer range.
    ///
    /// Because Orpheus represents time exactly using fractions (like `1/3`), performing complex
    /// transformations (like shifting and stretching) can sometimes cause the internal integer
    /// numerators and denominators to multiply beyond the capacity of a 128-bit integer.
    #[error("{operation} exceeded the supported range")]
    ArithmeticOverflow {
        /// A string identifying the mathematical operation that failed (e.g., `"addition"`, `"normalization"`).
        operation: &'static str,
    },
    /// A span was constructed with its start after its end.
    ///
    /// In Orpheus, time flows strictly forward. A [`TimeSpan`] must always have a non-negative duration,
    /// meaning its start boundary cannot occur temporally after its end boundary.
    #[error("time span start cannot exceed end")]
    InvalidSpan {
        /// The boundary where the span was requested to begin.
        start: Rational,
        /// The boundary where the span was requested to conclude.
        end: Rational,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_error_formats_invalid_denominator() {
        let err = PatternError::InvalidDenominator { denominator: 0 };
        assert_eq!(err.to_string(), "rational denominator cannot be zero");
    }

    #[test]
    fn pattern_error_formats_arithmetic_overflow() {
        let err = PatternError::ArithmeticOverflow {
            operation: "addition",
        };
        assert_eq!(err.to_string(), "addition exceeded the supported range");
    }

    #[test]
    fn pattern_error_formats_invalid_span() {
        let err = PatternError::InvalidSpan {
            start: Rational::new(2, 1).unwrap(),
            end: Rational::new(1, 1).unwrap(),
        };
        assert_eq!(err.to_string(), "time span start cannot exceed end");
    }
}
