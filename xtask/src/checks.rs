//! The required checks from DESIGN.md, run in order.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::{fmt, io};

/// The three required checks, in the order they run. Formatting runs first
/// because it is the cheapest, and tests last because they are the slowest.
///
/// `cargo fmt` runs without `--all`: on this virtual workspace it covers
/// exactly the members, while `--all` would also reach local path
/// dependencies such as the patched crates under `vendor/` (proposal 0009).
pub const REQUIRED_CHECKS: [&[&str]; 3] = [
    &["fmt", "--", "--check"],
    &[
        "clippy",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--",
        "-D",
        "warnings",
    ],
    &["test", "--workspace", "--all-features"],
];

/// A required check that did not pass.
#[derive(Debug)]
pub enum CheckError {
    /// Cargo could not be started.
    Spawn {
        /// The cargo arguments of the check.
        check: String,
        /// The operating-system error.
        source: io::Error,
    },
    /// The check ran and reported failure.
    Failed {
        /// The cargo arguments of the check.
        check: String,
        /// The exit code, absent when the process was terminated by a signal.
        code: Option<i32>,
    },
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { check, source } => {
                write!(f, "could not start `cargo {check}`: {source}")
            }
            Self::Failed {
                check,
                code: Some(code),
            } => write!(f, "`cargo {check}` failed with exit code {code}"),
            Self::Failed { check, code: None } => {
                write!(f, "`cargo {check}` was terminated")
            }
        }
    }
}

impl std::error::Error for CheckError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source, .. } => Some(source),
            Self::Failed { .. } => None,
        }
    }
}

/// Runs every required check in `workspace_root`, stopping at the first
/// failure. The checks inherit this process's standard streams, so their own
/// output is the report.
///
/// `cargo` is the cargo executable to run. Under `cargo xtask` it is the
/// pinned toolchain's cargo, which Cargo passes in the `CARGO` variable.
///
/// # Errors
///
/// Returns [`CheckError`] for the first check that cannot start or fails.
pub fn run_required_checks(cargo: &OsString, workspace_root: &Path) -> Result<(), CheckError> {
    for arguments in REQUIRED_CHECKS {
        let check = arguments.join(" ");
        let status = Command::new(cargo)
            .args(arguments)
            .current_dir(workspace_root)
            .status()
            .map_err(|source| CheckError::Spawn {
                check: check.clone(),
                source,
            })?;
        if !status.success() {
            return Err(CheckError::Failed {
                check,
                code: status.code(),
            });
        }
    }
    Ok(())
}
