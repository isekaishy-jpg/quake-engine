//! Reading a level: the engine's own format first, and then the game's.

use std::error::Error;
use std::sync::Arc;

use engine_assets::FileSystem;
use engine_compile::{compile_text, placeholder_texture};
use fixture_quake::{LoadedLevel, load_palette, spawn_point};

/// A level's text, in the engine's own format.
fn working_map_path(name: &str) -> String {
    format!("maps/{name}.emap")
}

/// Reads the level called `name`. A working map plays straight from its text,
/// with nothing baked (proposal 0067).
pub(crate) fn load_level(
    filesystem: &FileSystem,
    name: &str,
) -> Result<LoadedLevel, Box<dyn Error>> {
    let Ok(bytes) = filesystem.read(&working_map_path(name)) else {
        return fixture_quake::load_level(filesystem, name);
    };
    let text = std::str::from_utf8(&bytes)?;
    let mut textures = |material: &str| placeholder_texture(material);
    let map = Arc::new(compile_text(text, &mut textures)?);
    let palette = load_palette(filesystem)?;
    let start = spawn_point(&map);
    Ok(LoadedLevel {
        map,
        palette,
        start,
    })
}
