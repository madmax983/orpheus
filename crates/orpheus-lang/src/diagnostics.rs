//! Parser diagnostics for Orpheus source text.
//!
//! This module defines the error types emitted during the earlier phases of
//! Orpheus code processing: parsing, type inference, and module loading.
//! These errors are distinct from runtime evaluation errors ([`crate::EvalError`]),
//! which only occur after successful compilation.

use thiserror::Error;

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

#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error("{message}")]
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

impl From<ParseError> for LoadError {
    fn from(error: ParseError) -> Self {
        Self::new(error.to_string())
    }
}

impl From<TypeError> for LoadError {
    fn from(error: TypeError) -> Self {
        Self::new(error.to_string())
    }
}

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

#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error("{message}")]
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

#[derive(Clone, Debug, Eq, PartialEq, Error)]
#[error("{message}")]
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

    #[test]
    fn type_error_from_parse_error() {
        let parse_err = ParseError::new("mock parse error");
        let type_err: TypeError = parse_err.into();
        assert_eq!(type_err.to_string(), "mock parse error");
    }
}
