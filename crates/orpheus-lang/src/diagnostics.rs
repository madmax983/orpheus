//! Parser diagnostics for Orpheus source text.

use std::error::Error;
use std::fmt::{self, Display, Formatter};

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
