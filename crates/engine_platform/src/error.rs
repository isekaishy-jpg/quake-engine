//! Platform errors.

use std::fmt;

/// A failure of the application host.
#[derive(Debug)]
pub enum PlatformError {
    /// The host could not be created or failed while running.
    Host(String),
    /// The application's own startup failed. The message is its error.
    Startup(String),
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Host(message) => write!(f, "application host: {message}"),
            Self::Startup(message) => write!(f, "startup: {message}"),
        }
    }
}

impl std::error::Error for PlatformError {}
