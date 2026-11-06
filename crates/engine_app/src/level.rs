//! Reading a level: the engine's own format first, and then the game's.

use std::error::Error;
use std::sync::Arc;

use engine_assets::FileSystem;
use engine_compile::{PlayMap, apply_bake, attach_side_files, document_of, play_map};
use engine_map::SideFiles;
use fixture_quake::{LoadedLevel, load_palette, spawn_point};

/// A level's text, in the engine's own format.
fn working_map_path(name: &str) -> String {
    format!("maps/{name}.emap")
}

/// The level's own script, if it has one, beside its map.
pub(crate) fn level_scripts(filesystem: &FileSystem, name: &str) -> Vec<(String, String)> {
    let path = format!("maps/{name}.luau");
    filesystem
        .read(&path)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|text| (path, text))
        .into_iter()
        .collect()
}

/// Reads the level called `name`. A working map plays straight from its text,
/// with nothing baked (proposal 0067).
pub(crate) fn load_level(
    filesystem: &Arc<FileSystem>,
    name: &str,
) -> Result<LoadedLevel, Box<dyn Error>> {
    // Text while it's being worked on, and what compiling made once it isn't.
    let found = filesystem
        .read(&working_map_path(name))
        .or_else(|_| filesystem.read(&format!("maps/{name}.ebin")));
    let Ok(bytes) = found else {
        return fixture_quake::load_level(filesystem, name);
    };
    let document = document_of(&bytes)?;
    let PlayMap { mut map, hash, .. } = play_map(filesystem, name, &document)?;
    if let Ok(baked) = filesystem.read(&format!("maps/{name}.ebake")) {
        apply_bake(&mut map, &baked, hash)?;
        let source = Arc::clone(filesystem);
        let files: SideFiles = Arc::new(move |path| source.read(path).ok());
        attach_side_files(&mut map, name, &files);
    }
    let map = Arc::new(map);
    let palette = load_palette(filesystem)?;
    let start = spawn_point(&map);
    Ok(LoadedLevel {
        map,
        palette,
        start,
    })
}
