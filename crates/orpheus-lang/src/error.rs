use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};

use crate::diagnostics::{LoadError, ParseError, TypeError};
use crate::eval::EvalError;
use crate::export::RenderError;
use crate::pitch::PitchLiteralError;

/// A unified error type for the `orpheus-lang` crate.
///
/// This enum consolidates the various errors that can occur during the parsing,
/// typing, loading, evaluation, or rendering of Orpheus programs.
#[derive(Debug)]
pub enum Error {
    /// Occurs when source text violates the language grammar.
    Parse(ParseError),
    /// Occurs when a module fails static type checking.
    Type(TypeError),
    /// Occurs when loading a module from a file fails.
    Load(LoadError),
    /// Occurs when an expression fails to evaluate at runtime.
    Eval(EvalError),
    /// Occurs when an invalid pitch literal string is encountered.
    PitchLiteral(PitchLiteralError),
    /// Occurs during offline rendering or file exporting.
    Render(RenderError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => Display::fmt(error, f),
            Self::Type(error) => Display::fmt(error, f),
            Self::Load(error) => Display::fmt(error, f),
            Self::Eval(error) => Display::fmt(error, f),
            Self::PitchLiteral(error) => Display::fmt(error, f),
            Self::Render(error) => Display::fmt(error, f),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Parse(error) => Some(error),
            Self::Type(error) => Some(error),
            Self::Load(error) => Some(error),
            Self::Eval(error) => Some(error),
            Self::PitchLiteral(error) => Some(error),
            Self::Render(error) => Some(error),
        }
    }
}

impl From<ParseError> for Error {
    fn from(error: ParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<TypeError> for Error {
    fn from(error: TypeError) -> Self {
        Self::Type(error)
    }
}

impl From<LoadError> for Error {
    fn from(error: LoadError) -> Self {
        Self::Load(error)
    }
}

impl From<EvalError> for Error {
    fn from(error: EvalError) -> Self {
        Self::Eval(error)
    }
}

impl From<PitchLiteralError> for Error {
    fn from(error: PitchLiteralError) -> Self {
        Self::PitchLiteral(error)
    }
}

impl From<RenderError> for Error {
    fn from(error: RenderError) -> Self {
        Self::Render(error)
    }
}
