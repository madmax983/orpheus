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

    #[test]
    fn eval_error_from_type_error() {
        let type_err = crate::diagnostics::TypeError::new("mock type error");
        let err: EvalError = type_err.into();
        assert_eq!(err.to_string(), "mock type error");
    }

    #[test]
    fn eval_error_from_load_error() {
        let load_err = crate::diagnostics::LoadError::new("mock load error");
        let err: EvalError = load_err.into();
        assert_eq!(err.to_string(), "mock load error");
    }

    #[test]
    fn eval_error_from_parse_error() {
        let parse_err = crate::diagnostics::ParseError::new("mock parse error");
        let err: EvalError = parse_err.into();
        assert_eq!(err.to_string(), "mock parse error");
    }

    #[test]
    fn eval_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
        let err: EvalError = io_err.into();
        assert_eq!(err.to_string(), "file not found");

        let io_err2 =
            std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
        let err2: EvalError = io_err2.into();
        assert_eq!(err2.to_string(), "permission denied");

        let io_err3 = std::io::Error::other("other error");
        let err3: EvalError = io_err3.into();
        assert_eq!(err3.to_string(), "other error");
    }

    #[test]
    fn eval_error_from_fmt_error() {
        let fmt_err = std::fmt::Error;
        let err: EvalError = fmt_err.into();
        assert_eq!(
            err.to_string(),
            "an error occurred when formatting an argument"
        );
    }
}
