//! The two records of the build sequence and the reservation step.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{BuildLock, BuildNumber, BuildNumberError};

/// File name of the high-water mark in the git common directory.
const HIGH_WATER_FILE: &str = "quake-engine-build-number";
/// File name of the lock in the git common directory.
const LOCK_FILE: &str = "quake-engine-build.lock";

/// The build sequence of one repository: the tracked number, the high-water
/// mark and the lock that serialises reservations.
#[derive(Clone, Debug)]
pub struct BuildSequence {
    /// The tracked `BUILD_NUMBER` file.
    tracked: PathBuf,
    /// The high-water mark shared by every worktree of the repository.
    high_water: PathBuf,
    /// The lock file shared by every worktree of the repository.
    lock: PathBuf,
}

impl BuildSequence {
    /// The sequence of the repository at `repository_root`, with its shared
    /// records in the directory `git rev-parse --git-common-dir` reports.
    ///
    /// # Errors
    ///
    /// Returns [`BuildNumberError::GitCommonDir`] when git cannot run or does
    /// not recognise `repository_root` as a repository.
    pub fn for_repository(repository_root: &Path) -> Result<Self, BuildNumberError> {
        let output = Command::new("git")
            .args(["rev-parse", "--git-common-dir"])
            .current_dir(repository_root)
            .output()
            .map_err(|source| BuildNumberError::GitCommonDir {
                detail: source.to_string(),
            })?;
        if !output.status.success() {
            return Err(BuildNumberError::GitCommonDir {
                detail: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            });
        }
        let reported = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        // Git reports the common directory relative to the working directory
        // unless it lies elsewhere, so a relative answer is joined to the root.
        let common_dir = repository_root.join(reported);
        Ok(Self::new(
            &repository_root.join("BUILD_NUMBER"),
            &common_dir,
        ))
    }

    /// A sequence with its tracked number at `tracked` and its high-water mark
    /// and lock inside `shared_dir`.
    #[must_use]
    pub fn new(tracked: &Path, shared_dir: &Path) -> Self {
        Self {
            tracked: tracked.to_owned(),
            high_water: shared_dir.join(HIGH_WATER_FILE),
            lock: shared_dir.join(LOCK_FILE),
        }
    }

    /// The tracked build number: the last number reserved in this source tree.
    ///
    /// # Errors
    ///
    /// Returns [`BuildNumberError`] when the tracked file cannot be read or
    /// does not hold one decimal number.
    pub fn current(&self) -> Result<BuildNumber, BuildNumberError> {
        read_number(&self.tracked)
    }

    /// Takes the exclusive lock over the sequence. A package build holds it
    /// from reservation until compilation ends.
    ///
    /// # Errors
    ///
    /// Returns [`BuildNumberError::Locked`] when another process holds it.
    pub fn lock(&self) -> Result<BuildLock, BuildNumberError> {
        BuildLock::acquire(&self.lock)
    }

    /// Reserves the next build number and records it in both files.
    ///
    /// The high-water mark is written before the tracked file, so if the second
    /// write fails the number is still consumed and cannot be issued again.
    ///
    /// # Errors
    ///
    /// Returns [`BuildNumberError`] when `lock` does not belong to this
    /// sequence, a record cannot be read or written, or the sequence is
    /// exhausted.
    pub fn reserve(&self, lock: &BuildLock) -> Result<BuildNumber, BuildNumberError> {
        if lock.path() != self.lock {
            return Err(BuildNumberError::ForeignLock {
                held: lock.path().to_owned(),
                required: self.lock.clone(),
            });
        }
        let tracked = read_number(&self.tracked)?;
        let high_water = match read_number(&self.high_water) {
            Err(BuildNumberError::Io { source, .. })
                if source.kind() == io::ErrorKind::NotFound =>
            {
                // A fresh clone has no high-water mark yet; the tracked number
                // carries the sequence.
                BuildNumber::UNNUMBERED
            }
            other => other?,
        };
        let previous = tracked.max(high_water).value();
        let next = previous
            .checked_add(1)
            .map(BuildNumber::new)
            .ok_or(BuildNumberError::Exhausted)?;
        write_number(&self.high_water, next)?;
        write_number(&self.tracked, next)?;
        Ok(next)
    }
}

/// Reads a file holding one decimal number and an optional line ending.
fn read_number(path: &Path) -> Result<BuildNumber, BuildNumberError> {
    let text = fs::read_to_string(path).map_err(|source| BuildNumberError::Io {
        path: path.to_owned(),
        source,
    })?;
    let digits = text.trim_end_matches(['\r', '\n']);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(BuildNumberError::Malformed {
            path: path.to_owned(),
        });
    }
    digits
        .parse()
        .map(BuildNumber::new)
        .map_err(|_overflow| BuildNumberError::Malformed {
            path: path.to_owned(),
        })
}

/// Writes one decimal number and a line feed, replacing the file.
///
/// The number goes to a temporary file beside the record, which is then
/// renamed over it. A rename within one directory replaces the record
/// atomically, so a crash leaves the old number or the new one, never an
/// empty or partial record.
fn write_number(path: &Path, number: BuildNumber) -> Result<(), BuildNumberError> {
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    fs::write(&temporary, format!("{}\n", number.value())).map_err(|source| {
        BuildNumberError::Io {
            path: temporary.clone(),
            source,
        }
    })?;
    fs::rename(&temporary, path).map_err(|source| BuildNumberError::Io {
        path: path.to_owned(),
        source,
    })
}
