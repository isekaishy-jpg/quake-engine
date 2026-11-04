//! Logical file paths.

use crate::error::AssetError;

/// Turns `path` into the form archives use: lower-case, with forward slashes.
///
/// # Errors
///
/// Returns [`AssetError::InvalidPath`] for a path that is empty, absolute or
/// contains `..`, so a path from a map or a server can't leave the game's
/// data.
pub fn normalize_path(path: &str) -> Result<String, AssetError> {
    let normalized = path.replace('\\', "/").to_lowercase();
    let absolute = normalized.starts_with('/') || normalized.contains(':');
    let escapes = normalized.split('/').any(|part| part == "..");
    if normalized.is_empty() || absolute || escapes {
        return Err(AssetError::InvalidPath(path.to_owned()));
    }
    Ok(normalized)
}
