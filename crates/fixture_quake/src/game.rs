//! Quake's game directories, following `COM_AddGameDirectory` in `common.c`.

use std::fs;
use std::path::{Path, PathBuf};

use engine_assets::{Archive, AssetError, DirectoryArchive, FileSystem};

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

/// Mounts the rerelease's levels of `game` with coloured light, if it's there, over what's
/// mounted so far. Without the rerelease, levels stay white.
pub fn mount_coloured_levels(filesystem: &mut FileSystem, base: &Path, game: &str) {
    let path = base.join("rerelease").join(game).join("pak0.pak");
    if let Ok(pak) = PakArchive::open(&path) {
        filesystem.mount(Box::new(ColouredLevels::new(pak)));
    }
}

/// An archive's levels that have coloured light, and nothing else of it. A `.lit` file
/// matches the light in the level it was made with, not the original's, so a level and
/// its light come as a pair. Everything else stays as the game has it.
pub struct ColouredLevels<A: Archive> {
    /// What's read from.
    archive: A,
}

impl<A: Archive> ColouredLevels<A> {
    /// The coloured levels in `archive`.
    pub const fn new(archive: A) -> Self {
        Self { archive }
    }

    /// Whether `path` is a level with coloured light, or that light.
    fn keeps(&self, path: &str) -> bool {
        let Some(name) = path.strip_prefix("maps/") else {
            return false;
        };
        if name.ends_with(".lit") {
            return true;
        }
        name.strip_suffix(".bsp")
            .is_some_and(|stem| self.archive.contains(&format!("maps/{stem}.lit")))
    }
}

impl<A: Archive> Archive for ColouredLevels<A> {
    fn label(&self) -> &str {
        self.archive.label()
    }

    fn paths(&self) -> Vec<String> {
        let mut paths = self.archive.paths();
        paths.retain(|path| self.keeps(path));
        paths
    }

    fn contains(&self, path: &str) -> bool {
        self.keeps(path) && self.archive.contains(path)
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, AssetError> {
        if self.keeps(path) {
            self.archive.read(path)
        } else {
            Err(AssetError::NotFound(path.to_owned()))
        }
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
