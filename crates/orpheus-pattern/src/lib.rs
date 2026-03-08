//! Temporal pattern engine for Orpheus.

mod cycle;
mod event;
mod rational;
mod time;

pub use cycle::{CyclePattern, Pattern, PatternNode};
pub use event::Event;
pub use rational::Rational;
pub use time::TimeSpan;

use core::fmt;

/// Errors produced by the pattern core time model.
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
