//! Loose files in a directory.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::archive::Archive;
use crate::error::AssetError;

/// A directory of loose files.
#[derive(Clone, Debug)]
pub struct DirectoryArchive {
    /// The directory.
    root: PathBuf,
    /// The directory as text, for display.
    label: String,
}

impl DirectoryArchive {
    /// An archive over `root`.
    #[must_use]
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            label: root.display().to_string(),
        }
    }

    /// Finds the file on disk for `path`, whatever its case there. Archive
    /// paths are lower-case, and game files often aren't.
    fn resolve(&self, path: &str) -> Option<PathBuf> {
        let mut found = self.root.clone();
        for part in path.split('/') {
            let entry = fs::read_dir(&found).ok()?.flatten().find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(part)
            })?;
            found = entry.path();
        }
        Some(found)
    }
}

/// Adds every file under `directory` to `found`, with paths relative to the
/// archive and in archive form.
fn walk(directory: &Path, prefix: &str, found: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        let path = entry.path();
        if path.is_dir() {
            walk(&path, &format!("{prefix}{name}/"), found);
        } else {
            found.push(format!("{prefix}{name}"));
        }
    }
}

impl Archive for DirectoryArchive {
    fn label(&self) -> &str {
        &self.label
    }

    fn changed(&self, path: &str) -> Option<SystemTime> {
        fs::metadata(self.resolve(path)?).ok()?.modified().ok()
    }

    fn paths(&self) -> Vec<String> {
        let mut found = Vec::new();
        walk(&self.root, "", &mut found);
        found
    }

    fn contains(&self, path: &str) -> bool {
        self.resolve(path).is_some_and(|found| found.is_file())
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, AssetError> {
        let found = self
            .resolve(path)
            .ok_or_else(|| AssetError::NotFound(path.to_owned()))?;
        fs::read(found).map_err(|source| {
            if source.kind() == ErrorKind::NotFound {
                AssetError::NotFound(path.to_owned())
            } else {
                AssetError::Io {
                    path: path.to_owned(),
                    source,
                }
            }
        })
    }
}
