//! Package build numbers.
//!
//! DESIGN.md requires a monotonically increasing build number for packaged
//! builds, reserved before compiling and never reused. Two records keep the
//! sequence:
//! - the tracked `BUILD_NUMBER` file at the repository root, which carries the
//!   sequence through clones and source history;
//! - a high-water mark in Git's common directory, which survives switching to
//!   an older branch, linked worktrees and failed package builds.
//!
//! A reservation takes the larger of the two, adds one, and writes both before
//! anything is compiled, under an exclusive lock that a packaging step holds
//! through compilation. A failed build therefore leaves a gap, never a reused
//! number. Zero means the source state before the first numbered package.

mod lock;
mod sequence;

pub use lock::BuildLock;
pub use sequence::BuildSequence;

use std::fmt;
use std::io;
use std::path::PathBuf;

/// A package build number. It is displayed with at least six digits, which is
/// a minimum width, not a limit on the sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BuildNumber(u32);

impl BuildNumber {
    /// The number of the source state before the first numbered package.
    pub const UNNUMBERED: Self = Self(0);

    /// Wraps a raw build number.
    #[must_use]
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// The raw build number.
    #[must_use]
    pub fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for BuildNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:06}", self.0)
    }
}

/// A failure to read, lock or advance the build sequence.
#[derive(Debug)]
pub enum BuildNumberError {
    /// A sequence file could not be read or written.
    Io {
        /// The file involved.
        path: PathBuf,
        /// The operating-system error.
        source: io::Error,
    },
    /// A sequence file does not hold one decimal number.
    Malformed {
        /// The file involved.
        path: PathBuf,
    },
    /// Another reservation or package build holds the lock.
    Locked {
        /// The lock file. If no package build is running, a crashed one left
        /// it behind and it can be deleted by hand.
        path: PathBuf,
    },
    /// A reservation was attempted with the lock of a different sequence.
    /// This is a programming error in the caller, not contention.
    ForeignLock {
        /// The lock file the caller holds.
        held: PathBuf,
        /// The lock file this sequence requires.
        required: PathBuf,
    },
    /// The sequence has reached `u32::MAX`.
    Exhausted,
    /// Git could not report the repository's common directory.
    GitCommonDir {
        /// What went wrong.
        detail: String,
    },
}

impl fmt::Display for BuildNumberError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Malformed { path } => {
                write!(f, "{} does not hold one build number", path.display())
            }
            Self::Locked { path } => write!(
                f,
                "the build sequence is locked by {}; if no package build is running, delete it",
                path.display()
            ),
            Self::ForeignLock { held, required } => write!(
                f,
                "the reservation holds the lock {} but this sequence requires {}",
                held.display(),
                required.display()
            ),
            Self::Exhausted => f.write_str("the build sequence is exhausted"),
            Self::GitCommonDir { detail } => {
                write!(f, "could not find the git common directory: {detail}")
            }
        }
    }
}

impl std::error::Error for BuildNumberError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Malformed { .. }
            | Self::Locked { .. }
            | Self::ForeignLock { .. }
            | Self::Exhausted
            | Self::GitCommonDir { .. } => None,
        }
    }
}
