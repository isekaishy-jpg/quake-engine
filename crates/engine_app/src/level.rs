//! Reading a level: the engine's own format first, and then the game's.

use std::error::Error;
use std::sync::Arc;

use engine_assets::FileSystem;
use engine_compile::{MaterialLibrary, compile_text};
use engine_map::Texture;
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
    let mut library = MaterialLibrary::new(filesystem);
    let mut failure = None;
    let mut textures = |material: &str| {
        library.load(material).unwrap_or_else(|error| {
            failure = Some(error);
            Texture::default()
        })
    };
    let map = compile_text(text, &mut textures)?;
    if let Some(error) = failure {
        return Err(error.into());
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
