use orpheus_pattern::PatternError;
use thiserror::Error;

use crate::diagnostics::ParseError;

/// Runtime errors that occur while evaluating an Orpheus expression.
///
/// Unlike parser or type-checker errors, `EvalError` occurs during the actual
/// mathematical or temporal execution of the pattern.
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum EvalError {
    /// An arbitrary runtime error message string.
    ///
    /// This is a fallback variant for dynamically generated evaluation errors
    /// (e.g. division by zero, capacity overflows) that don't fit into a specific domain type.
    #[error("{message}")]
    Message {
        /// The textual description of the error.
        message: Box<str>,
    },

    /// An error that occurred while parsing a dynamic evaluation string.
    ///
    /// This happens when source code provided to `eval_module` contains syntax errors.
    #[error(transparent)]
    Parse(#[from] ParseError),

    /// A type checking error during expression evaluation or function application.
    ///
    /// This occurs when an expression tries to apply a function to an invalid
    /// variable type (e.g., trying to shift a `Value::Function`).
    #[error(transparent)]
    Type(#[from] crate::diagnostics::TypeError),

    /// An error encountered when loading an external resource.
    ///
    /// This is typically emitted when parsing a file or a sample directory fails.
    #[error(transparent)]
    Load(#[from] crate::diagnostics::LoadError),

    /// An error parsing a named pitch literal into semitones.
    ///
    /// This happens if an identifier resolves to an invalid note name (like `C#99`).
    #[error(transparent)]
    Pitch(#[from] crate::pitch::PitchLiteralError),

    /// A downcasting bounds error for integer representations.
    ///
    /// Occurs when explicitly converting numbers like cycle repeats or bounds
    /// into usize or u64 and the value is out of range.
    #[error(transparent)]
    TryFromInt(#[from] std::num::TryFromIntError),

    /// An error parsing a string into an integer.
    #[error(transparent)]
    ParseInt(#[from] std::num::ParseIntError),

    /// An underlying temporal error from pattern operations.
    ///
    /// Examples include attempting a rational division by zero or invalid shifts
    /// in explicit-time streams.
    #[error(transparent)]
    Pattern(#[from] PatternError),
}

impl EvalError {
    /// Creates a new `EvalError` with the given message.
    ///
    /// The message explains what went wrong during runtime evaluation.
    ///
    /// Common causes for `EvalError` include:
    /// - Out-of-bounds numeric parameters.
    /// - Arithmetic overflow during explicit time-shifts.
    /// - Applying functions to invalid types.
    ///
    /// # Examples
    ///
    /// ```
    /// use orpheus_lang::EvalError;
    ///
    /// let err = EvalError::new("division by zero");
    /// assert_eq!(err.to_string(), "division by zero");
    /// ```
    pub fn new(message: impl Into<Box<str>>) -> Self {
        Self::Message {
            message: message.into(),
        }
    }
}

impl From<std::io::Error> for EvalError {
    fn from(error: std::io::Error) -> Self {
        let message = match error.kind() {
            std::io::ErrorKind::NotFound => "file not found".to_owned(),
            std::io::ErrorKind::PermissionDenied => "permission denied".to_owned(),
            _ => error.to_string(),
        };
        Self::new(message)
    }
}

impl From<std::fmt::Error> for EvalError {
    fn from(_error: std::fmt::Error) -> Self {
        Self::new("an error occurred when formatting an argument")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use std::io;

    #[test]
    fn test_from_io_error_not_found() {
        let err = io::Error::new(io::ErrorKind::NotFound, "test");
        let eval_err: EvalError = err.into();
        assert_eq!(eval_err.to_string(), "file not found");
    }

    #[test]
    fn test_from_io_error_permission_denied() {
        let err = io::Error::new(io::ErrorKind::PermissionDenied, "test");
        let eval_err: EvalError = err.into();
        assert_eq!(eval_err.to_string(), "permission denied");
    }

    #[test]
    fn test_from_io_error_other() {
        let err = io::Error::new(io::ErrorKind::Other, "custom io error");
        let eval_err: EvalError = err.into();
        assert_eq!(eval_err.to_string(), "custom io error");
    }

    #[test]
    fn test_from_fmt_error() {
        let err = fmt::Error;
        let eval_err: EvalError = err.into();
        assert_eq!(
            eval_err.to_string(),
            "an error occurred when formatting an argument"
        );
    }

    #[test]
    fn test_from_parse_error() {
        let err = crate::diagnostics::ParseError::new("mock parse error");
        let eval_err: EvalError = err.into();
        assert!(matches!(eval_err, EvalError::Parse(_)));
        assert_eq!(eval_err.to_string(), "mock parse error");
    }

    #[test]
    fn test_from_type_error() {
        let err = crate::diagnostics::TypeError::new("mock type error");
        let eval_err: EvalError = err.into();
        assert!(matches!(eval_err, EvalError::Type(_)));
        assert_eq!(eval_err.to_string(), "mock type error");
    }

    #[test]
    fn test_from_load_error() {
        let err = crate::diagnostics::LoadError::new("mock load error");
        let eval_err: EvalError = err.into();
        assert!(matches!(eval_err, EvalError::Load(_)));
        assert_eq!(eval_err.to_string(), "mock load error");
    }

    #[test]
    fn test_from_pitch_literal_error() {
        let err = crate::pitch::PitchLiteralError::new(
            "invalid named pitch literal: 'C#99' - invalid octave",
        );
        let eval_err: EvalError = err.into();
        assert!(matches!(eval_err, EvalError::Pitch(_)));
        assert_eq!(
            eval_err.to_string(),
            "invalid named pitch literal: 'C#99' - invalid octave"
        );
    }

    #[test]
    fn test_from_try_from_int_error() {
        let err: Result<u8, _> = 1000u16.try_into();
        let eval_err: EvalError = err.unwrap_err().into();
        assert!(matches!(eval_err, EvalError::TryFromInt(_)));
        assert!(eval_err.to_string().contains("out of range"));
    }

    #[test]
    fn test_from_parse_int_error() {
        let err = "abc".parse::<i32>().unwrap_err();
        let eval_err: EvalError = err.into();
        assert!(matches!(eval_err, EvalError::ParseInt(_)));
        assert!(eval_err.to_string().contains("invalid digit"));
    }

    #[test]
    fn test_from_pattern_error() {
        let err = PatternError::InvalidDenominator { denominator: 0 };
        let eval_err: EvalError = err.into();
        assert!(matches!(eval_err, EvalError::Pattern(_)));
        assert_eq!(eval_err.to_string(), "rational denominator cannot be zero");
    }
}
