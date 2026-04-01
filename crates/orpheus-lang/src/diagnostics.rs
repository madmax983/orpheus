//! Parser diagnostics for Orpheus source text.
//!
//! This module defines the error types emitted during the earlier phases of
//! Orpheus code processing: parsing, type inference, and module loading.
//! These errors are distinct from runtime evaluation errors ([`crate::EvalError`]),
//! which only occur after successful compilation.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

/// A parser error produced while reading Orpheus source.
///
/// This error is returned by [`crate::parse_module`] when the source text
/// violates the Orpheus language grammar.
///
/// # Causes
///
/// Common causes include:
/// - Unbalanced parentheses or braces.
/// - Missing or unexpected tokens (like a dangling comma or pipe).
/// - Malformed literals (e.g., an unclosed string quote).
///
/// # Examples
///
/// ```
/// use orpheus_lang::parse_module;
///
/// // Missing a closing parenthesis.
/// let result = parse_module("song = fast(2, bd");
/// assert!(result.is_err());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    message: Box<str>,
}

impl ParseError {
    pub(crate) fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ParseError {}

/// A type inference error produced while analyzing Orpheus source.
///
/// This error is returned by [`crate::infer_module`] when a parsed module fails
/// the static type check logic. In Orpheus, static typing enforces constraints
/// like matching sequence elements or valid arguments for built-in functions.
///
/// # Causes
///
/// Common causes include:
/// - A heterogeneous sequence like `bd 123`. Sequences must contain either all
///   sample patterns or all number patterns.
/// - Passing a number literal to a function expecting a pattern of numbers.
///
/// # Examples
///
/// ```
/// use orpheus_lang::{parse_module, infer_module, ReplMode};
///
/// let source = "song = fast(bd, sn)";
/// // The `fast` transform expects a numeric multiplier as its first argument,
/// // not a sample pattern like `bd`.
/// let typed_result = infer_module(source, ReplMode::Strict);
/// assert!(typed_result.is_err());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeError {
    message: Box<str>,
}

impl TypeError {
    pub(crate) fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for TypeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for TypeError {}

/// A file-loading error produced while resolving strict `.ode` modules.
///
/// This error is returned by [`crate::load_file_strict`] when attempting to read
/// a file from disk fails, or if the file contains invalid Orpheus source
/// (parsing errors) or type mismatches (type inference errors).
///
/// # Causes
///
/// Common causes include:
/// - The file path provided does not exist.
/// - The application does not have permission to read the file.
/// - The `.ode` file contains invalid syntax (results in a `ParseError` wrapped inside).
/// - The `.ode` file contains type errors (results in a `TypeError` wrapped inside).
///
/// # Examples
///
/// ```
/// use orpheus_lang::load_file_strict;
///
/// // Trying to load a non-existent file path will return a `LoadError`.
/// let result = load_file_strict("this_file_does_not_exist.ode");
/// assert!(result.is_err());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadError {
    message: Box<str>,
}

impl LoadError {
    pub(crate) fn new(message: impl Into<Box<str>>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for LoadError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for LoadError {}

/// Runtime evaluation error for bootstrap Orpheus modules.
///
/// `EvalError` occurs when an expression fails to evaluate at runtime.
/// In Orpheus, evaluation errors often stem from invalid arithmetic on rational
/// time domains (like dividing by zero), out-of-bounds parameters, or attempting
/// to use an unsupported operation on a pattern. Orpheus patterns operate in an
/// exact, bounded rational time domain, so overflows during shifts or scaling
/// can result in an `EvalError`.
///
/// # Examples
///
/// An `EvalError` provides an error message indicating what went wrong:
///
/// ```
/// use orpheus_lang::EvalError;
///
/// let err = EvalError::new("decimal literal exceeded the supported range");
/// assert_eq!(err.to_string(), "decimal literal exceeded the supported range");
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalError {
    message: Box<str>,
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
        Self {
            message: message.into(),
        }
    }
}

impl Display for EvalError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for EvalError {}

impl From<ParseError> for EvalError {
    fn from(error: ParseError) -> Self {
        Self::new(error.to_string())
    }
}

/// Error raised while rendering an Orpheus sample pattern to an audio file.
#[derive(Debug)]
pub enum RenderError {
    Eval(EvalError),
    Audio(orpheus_dsp::OfflineRenderError),
}

impl Display for RenderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eval(error) => Display::fmt(error, formatter),
            Self::Audio(error) => Display::fmt(error, formatter),
        }
    }
}

impl Error for RenderError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Eval(error) => Some(error),
            Self::Audio(error) => Some(error),
        }
    }
}

impl From<EvalError> for RenderError {
    fn from(error: EvalError) -> Self {
        Self::Eval(error)
    }
}

impl From<orpheus_dsp::OfflineRenderError> for RenderError {
    fn from(error: orpheus_dsp::OfflineRenderError) -> Self {
        Self::Audio(error)
    }
}

/// A unified error type for Orpheus language operations.
///
/// This enum encapsulates all possible errors that can occur during
/// parsing, type checking, loading, evaluating, and rendering Orpheus code.
#[derive(Debug)]
pub enum LangError {
    Parse(ParseError),
    Type(TypeError),
    Load(LoadError),
    Eval(EvalError),
    Render(RenderError),
}

impl Display for LangError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => Display::fmt(error, formatter),
            Self::Type(error) => Display::fmt(error, formatter),
            Self::Load(error) => Display::fmt(error, formatter),
            Self::Eval(error) => Display::fmt(error, formatter),
            Self::Render(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for LangError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            Self::Type(error) => Some(error),
            Self::Load(error) => Some(error),
            Self::Eval(error) => Some(error),
            Self::Render(error) => Some(error),
        }
    }
}

impl From<ParseError> for LangError {
    fn from(error: ParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<TypeError> for LangError {
    fn from(error: TypeError) -> Self {
        Self::Type(error)
    }
}

impl From<LoadError> for LangError {
    fn from(error: LoadError) -> Self {
        Self::Load(error)
    }
}

impl From<EvalError> for LangError {
    fn from(error: EvalError) -> Self {
        Self::Eval(error)
    }
}

impl From<RenderError> for LangError {
    fn from(error: RenderError) -> Self {
        Self::Render(error)
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_display() {
        let err = ParseError::new("syntax error");
        assert_eq!(err.to_string(), "syntax error");
    }

    #[test]
    fn type_error_display() {
        let err = TypeError::new("type mismatch");
        assert_eq!(err.to_string(), "type mismatch");
    }

    #[test]
    fn load_error_display() {
        let err = LoadError::new("file not found");
        assert_eq!(err.to_string(), "file not found");
    }
}
