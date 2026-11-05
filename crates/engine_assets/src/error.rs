//! Asset errors.

use std::{fmt, io};

/// A failure to find or read a file.
#[derive(Debug)]
pub enum AssetError {
    /// No archive holds this path.
    NotFound(String),
    /// The path isn't allowed: it's empty, absolute or contains `..`.
    InvalidPath(String),
    /// The operating system refused the read.
    Io {
        /// What was being read.
        path: String,
        /// The operating-system error.
        source: io::Error,
    },
    /// An archive's contents are damaged.
    Corrupt {
        /// The archive.
        archive: String,
        /// What's wrong.
        reason: String,
    },
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "file not found: {path}"),
            Self::InvalidPath(path) => write!(f, "invalid path: {path}"),
            Self::Io { path, source } => write!(f, "could not read {path}: {source}"),
            Self::Corrupt { archive, reason } => write!(f, "damaged archive {archive}: {reason}"),
        }
    }
}

impl std::error::Error for AssetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
