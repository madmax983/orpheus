use orpheus_dsp::OfflineRenderError;
use orpheus_pattern::PatternError;
use std::error::Error as StdError;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Error {
    Parse(Box<str>),
    Type(Box<str>),
    Load(Box<str>),
    Eval(Box<str>),
    Render(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(msg) | Self::Type(msg) | Self::Load(msg) | Self::Eval(msg) => {
                f.write_str(msg)
            }
            Self::Render(msg) => f.write_str(msg),
        }
    }
}

impl StdError for Error {}

impl Error {
    pub fn parse(msg: impl Into<Box<str>>) -> Self {
        Self::Parse(msg.into())
    }
    pub fn type_err(msg: impl Into<Box<str>>) -> Self {
        Self::Type(msg.into())
    }
    pub fn load(msg: impl Into<Box<str>>) -> Self {
        Self::Load(msg.into())
    }
    pub fn eval(msg: impl Into<Box<str>>) -> Self {
        Self::Eval(msg.into())
    }
}

impl From<std::num::TryFromIntError> for Error {
    fn from(err: std::num::TryFromIntError) -> Self {
        Self::eval(err.to_string())
    }
}
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::eval(err.to_string())
    }
}
impl From<std::fmt::Error> for Error {
    fn from(err: std::fmt::Error) -> Self {
        Self::eval(err.to_string())
    }
}
impl From<PatternError> for Error {
    fn from(err: PatternError) -> Self {
        Self::eval(err.to_string())
    }
}
impl From<OfflineRenderError> for Error {
    fn from(err: OfflineRenderError) -> Self {
        Self::Render(err.to_string())
    }
}
