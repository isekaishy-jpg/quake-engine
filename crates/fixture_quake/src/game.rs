//! Quake's game directories, following `COM_AddGameDirectory` in `common.c`.

use std::fs;
use std::path::{Path, PathBuf};

use engine_assets::{AssetError, DirectoryArchive, FileSystem};

use crate::pak::PakArchive;

/// Finds `name` in `directory`, ignoring case, since Steam's files are
/// upper-case.
fn find_ignoring_case(directory: &Path, name: &str) -> Option<PathBuf> {
    fs::read_dir(directory)
        .ok()?
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
        })
        .map(|entry| entry.path())
}

/// Mounts the rerelease's loose files for `game`, which is where Steam keeps
/// its music, if there are any. Mount this first, so it's searched last.
pub fn mount_music(filesystem: &mut FileSystem, base: &Path, game: &str) {
    let directory = base.join("rerelease").join(game);
    if directory.is_dir() {
        filesystem.mount(Box::new(DirectoryArchive::new(&directory)));
    }
}

/// Mounts a game directory: its loose files, then `pak0.pak`, `pak1.pak` and
/// so on until one is missing. Each goes in front, so paks override loose
/// files in the same directory, as in the original.
///
/// # Errors
///
/// Returns [`AssetError::NotFound`] if `directory` doesn't exist, and any
/// error from opening a pak.
pub fn mount_game(filesystem: &mut FileSystem, directory: &Path) -> Result<(), AssetError> {
    if !directory.is_dir() {
        return Err(AssetError::NotFound(directory.display().to_string()));
    }
    filesystem.mount(Box::new(DirectoryArchive::new(directory)));
    for number in 0.. {
        let Some(path) = find_ignoring_case(directory, &format!("pak{number}.pak")) else {
            break;
        };
        filesystem.mount(Box::new(PakArchive::open(&path)?));
    }
    Ok(())
}
