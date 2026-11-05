//! What can go wrong compiling.

use std::error::Error;
use std::fmt;

use engine_format::FormatError;

/// Why a map couldn't be compiled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
    /// The document couldn't be read.
    Format(FormatError),
    /// The document isn't a map.
    NotAMap(String),
    /// A block says something that makes no sense.
    Bad(String),
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Format(error) => error.fmt(f),
            Self::NotAMap(kind) => write!(f, "expected a map, found {kind}"),
            Self::Bad(message) => f.write_str(message),
        }
    }
}

impl Error for CompileError {}

impl From<FormatError> for CompileError {
    fn from(error: FormatError) -> Self {
        Self::Format(error)
    }
}
