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

use core::fmt;

/// Errors produced by the pattern core time model.
///
/// # Examples
///
/// ```
/// use orpheus_pattern::{PatternError, Rational, TimeSpan};
///
/// // Example of an invalid denominator error.
/// let err = Rational::new(1, 0).unwrap_err();
/// assert!(matches!(err, PatternError::InvalidDenominator { .. }));
///
/// // Example of an invalid span error.
/// let start = Rational::new(2, 1).unwrap();
/// let end = Rational::new(1, 1).unwrap();
/// let err = TimeSpan::new(start, end).unwrap_err();
/// assert!(matches!(err, PatternError::InvalidSpan { .. }));
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PatternError {
    /// A rational value was constructed with a zero denominator.
    InvalidDenominator { denominator: i64 },
    /// A checked rational operation exceeded the supported integer range.
    ArithmeticOverflow { operation: &'static str },
    /// A span was constructed with its start after its end.
    InvalidSpan { start: Rational, end: Rational },
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDenominator { .. } => f.write_str("rational denominator cannot be zero"),
            Self::ArithmeticOverflow { operation } => {
                write!(f, "{operation} exceeded the supported range")
            }
            Self::InvalidSpan { .. } => f.write_str("time span start cannot exceed end"),
        }
    }
}

impl std::error::Error for PatternError {}

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
