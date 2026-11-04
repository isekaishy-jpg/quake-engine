//! Builds the workspace model from `cargo metadata` and the manifests.

use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use cargo_metadata::{DependencyKind, MetadataCommand, Package};

use super::manifest::inherits_workspace_lints;
use super::{Dependency, Kind, WorkspacePackage};

/// Failure to read the workspace.
#[derive(Debug)]
pub enum LoadError {
    /// `cargo metadata` failed or its output could not be read.
    Metadata(cargo_metadata::Error),
    /// A member's manifest could not be read.
    Manifest {
        /// The manifest path.
        path: PathBuf,
        /// The operating-system error.
        source: io::Error,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Metadata(source) => write!(f, "cargo metadata failed: {source}"),
            Self::Manifest { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Metadata(source) => Some(source),
            Self::Manifest { source, .. } => Some(source),
        }
    }
}

/// Reads the workspace members of the manifest at `manifest_path` with
/// `cargo metadata --no-deps`, which reports declared dependencies without
/// resolving or downloading anything, and reads each member's manifest for
/// the facts Cargo does not report.
///
/// # Errors
///
/// Returns [`LoadError`] when cargo fails, its output cannot be read, or a
/// member's manifest cannot be read.
pub fn load_workspace(
    cargo: &OsStr,
    manifest_path: &Path,
) -> Result<Vec<WorkspacePackage>, LoadError> {
    let metadata = MetadataCommand::new()
        .cargo_path(cargo)
        .manifest_path(manifest_path)
        .no_deps()
        .exec()
        .map_err(LoadError::Metadata)?;
    metadata
        .workspace_packages()
        .into_iter()
        .map(convert_package)
        .collect()
}

/// Converts one `cargo metadata` package into the model.
fn convert_package(package: &Package) -> Result<WorkspacePackage, LoadError> {
    let manifest_path = package.manifest_path.as_std_path();
    let manifest = fs::read_to_string(manifest_path).map_err(|source| LoadError::Manifest {
        path: manifest_path.to_owned(),
        source,
    })?;
    Ok(WorkspacePackage {
        name: package.name.to_string(),
        edition: package.edition.as_str().to_owned(),
        rust_version: package.rust_version.as_ref().map(|version| {
            // Cargo reports `1.98` as `1.98.0`; the rule is stated as
            // Cargo.toml spells it.
            let text = version.to_string();
            text.strip_suffix(".0").map_or(text.clone(), str::to_owned)
        }),
        // `publish = false` is reported as an empty registry list.
        publishable: package
            .publish
            .as_ref()
            .is_none_or(|registries| !registries.is_empty()),
        inherits_workspace_lints: inherits_workspace_lints(&manifest),
        dependencies: package
            .dependencies
            .iter()
            .map(|dependency| Dependency {
                name: dependency.name.clone(),
                kind: match dependency.kind {
                    DependencyKind::Development => Kind::Development,
                    DependencyKind::Build => Kind::Build,
                    // `Unknown` is reserved for kinds newer than this
                    // cargo_metadata; treating it as normal applies the
                    // strictest rules.
                    DependencyKind::Normal | DependencyKind::Unknown => Kind::Normal,
                },
                features: dependency.features.clone(),
            })
            .collect(),
        feature_enables: package.features.values().flatten().cloned().collect(),
    })
}
