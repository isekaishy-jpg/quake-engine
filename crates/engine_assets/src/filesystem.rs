//! The search path.

use std::collections::BTreeSet;

use crate::archive::Archive;
use crate::error::AssetError;
use crate::path::normalize_path;

/// An ordered search path: the most recently mounted archive wins.
#[derive(Default)]
pub struct FileSystem {
    /// Archives, lowest priority first.
    archives: Vec<Box<dyn Archive>>,
}

impl FileSystem {
    /// An empty search path.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `archive` in front of everything already mounted.
    pub fn mount(&mut self, archive: Box<dyn Archive>) {
        self.archives.push(archive);
    }

    /// Reads `path` from the first archive that holds it.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::InvalidPath`] for a bad path,
    /// [`AssetError::NotFound`] if no archive holds it, and any error from
    /// reading it.
    pub fn read(&self, path: &str) -> Result<Vec<u8>, AssetError> {
        let normalized = normalize_path(path)?;
        self.archives
            .iter()
            .rev()
            .find(|archive| archive.contains(&normalized))
            .ok_or_else(|| AssetError::NotFound(normalized.clone()))?
            .read(&normalized)
    }

    /// Reads the file with id `id`, from the last archive that knows it.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::NotFound`] if no archive knows it, and other errors if
    /// reading fails.
    pub fn read_id(&self, id: u32) -> Result<Vec<u8>, AssetError> {
        let path = self
            .archives
            .iter()
            .rev()
            .find_map(|archive| archive.path_of_id(id))
            .ok_or_else(|| AssetError::NotFound(format!("file id {id}")))?;
        self.read(&path)
    }

    /// Whether any archive holds `path`.
    #[must_use]
    pub fn exists(&self, path: &str) -> bool {
        normalize_path(path)
            .is_ok_and(|normalized| self.archives.iter().any(|a| a.contains(&normalized)))
    }

    /// Every file on the search path starting with `prefix`, sorted, each
    /// listed once.
    #[must_use]
    pub fn list(&self, prefix: &str) -> Vec<String> {
        let prefix = prefix.to_lowercase();
        let unique: BTreeSet<String> = self
            .archives
            .iter()
            .flat_map(|archive| archive.paths())
            .filter(|path| path.starts_with(&prefix))
            .collect();
        unique.into_iter().collect()
    }

    /// The label of the archive that would serve `path`.
    #[must_use]
    pub fn location(&self, path: &str) -> Option<&str> {
        let normalized = normalize_path(path).ok()?;
        self.archives
            .iter()
            .rev()
            .find(|archive| archive.contains(&normalized))
            .map(|archive| archive.label())
    }

    /// The archives' labels, highest priority first.
    #[must_use]
    pub fn search_path(&self) -> Vec<&str> {
        self.archives.iter().rev().map(|a| a.label()).collect()
    }
}
