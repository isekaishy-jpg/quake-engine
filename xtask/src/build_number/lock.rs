//! The exclusive lock over the build sequence.

use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use super::BuildNumberError;

/// Proof that this process holds the build sequence lock. The lock is a file
/// created with `create_new`, which the file system makes atomic, so two
/// processes cannot both hold it. Dropping the guard deletes the file.
#[derive(Debug)]
pub struct BuildLock {
    /// The lock file this guard owns.
    path: PathBuf,
}

impl BuildLock {
    /// Takes the lock at `path`, failing at once if it is held: waiting could
    /// hang behind a crashed package build, and the caller can retry.
    pub(super) fn acquire(path: &Path) -> Result<Self, BuildNumberError> {
        match OpenOptions::new().write(true).create_new(true).open(path) {
            Ok(_) => Ok(Self {
                path: path.to_owned(),
            }),
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                Err(BuildNumberError::Locked {
                    path: path.to_owned(),
                })
            }
            Err(source) => Err(BuildNumberError::Io {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// The lock file this guard owns.
    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for BuildLock {
    fn drop(&mut self) {
        // A lock file that cannot be removed is reported as `Locked` by the
        // next reservation, which names the file; there is nobody to report
        // the failure to here.
        let _removed = fs::remove_file(&self.path);
    }
}
