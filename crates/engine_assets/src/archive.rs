//! Anything that holds files.

use crate::error::AssetError;

/// A source of files. Paths are normalized by [`crate::normalize_path`]. Files
/// can be read from any thread, as tiles are while a game goes on.
pub trait Archive: Send + Sync {
    /// A name to show for this archive.
    fn label(&self) -> &str;

    /// Every file it holds.
    fn paths(&self) -> Vec<String>;

    /// Whether it holds `path`.
    fn contains(&self, path: &str) -> bool;

    /// The path of file id `id`, for an archive that knows ids.
    fn path_of_id(&self, _id: u32) -> Option<String> {
        None
    }

    /// Reads `path`.
    ///
    /// # Errors
    ///
    /// Returns [`AssetError::NotFound`] if it isn't here, and other errors if
    /// reading fails.
    fn read(&self, path: &str) -> Result<Vec<u8>, AssetError>;
}
