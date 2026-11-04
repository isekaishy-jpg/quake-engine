//! What can go wrong with audio.

use std::fmt;

/// Music that wouldn't play.
#[derive(Debug)]
pub struct AudioError(pub(crate) String);

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AudioError {}
