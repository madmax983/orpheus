//! Parser diagnostics for Orpheus source text.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

use orpheus_dsp::OfflineRenderError;

/// Runtime evaluation error for bootstrap Orpheus modules.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvalError {
    message: Box<str>,
}

impl EvalError {
    pub(crate) fn new(message: impl Into<Box<str>>) -> Self {
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

/// Error raised while rendering an Orpheus sample pattern to an audio file.
#[derive(Debug)]
pub enum RenderError {
    Eval(EvalError),
    Audio(OfflineRenderError),
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

impl From<OfflineRenderError> for RenderError {
    fn from(error: OfflineRenderError) -> Self {
        Self::Audio(error)
    }
}

/// A parser error produced while reading Orpheus source.
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

impl From<ParseError> for EvalError {
    fn from(error: ParseError) -> Self {
        Self::new(error.to_string())
    }
}

/// A type inference error produced while analyzing Orpheus source.
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
