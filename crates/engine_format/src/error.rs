//! What can go wrong reading a document.

use std::error::Error;
use std::fmt;

/// Why a document couldn't be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
    /// Text that doesn't parse, and the line it's on, counted from 1.
    Text {
        /// The line.
        line: usize,
        /// What was wrong.
        message: String,
    },
    /// Binary that doesn't parse.
    Binary(String),
    /// A version newer than this reader knows.
    TooNew(u32),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text { line, message } => write!(f, "line {line}: {message}"),
            Self::Binary(message) => write!(f, "bad binary document: {message}"),
            Self::TooNew(version) => write!(f, "format version {version} is too new"),
        }
    }
}

impl Error for FormatError {}
