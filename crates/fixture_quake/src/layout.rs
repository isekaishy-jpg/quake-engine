//! Where Quake keeps things, and the rules its console sets.

use std::error::Error;
use std::sync::Arc;

use engine_assets::FileSystem;
use engine_map::Map;

use crate::bsp::{SpawnPoint, import_map, spawn_point};
use crate::texture::Palette;

/// The game directory used when `-game` isn't given.
pub const BASE_GAME: &str = "id1";

/// What the console runs to start the game.
pub const STARTUP_SCRIPT: &str = "quake.rc";

/// The game's own settings.
pub const DEFAULT_CONFIG: &str = "default.cfg";

/// The palette everything is drawn in.
const PALETTE: &str = "gfx/palette.lmp";

/// Where the status bar's pictures are.
pub const PICTURE_WAD: &str = "gfx.wad";

/// The console's background.
pub const BACKGROUND_PICTURE: &str = "gfx/conback.lmp";

/// The sound of someone talking.
pub const TALK_SOUND: &str = "misc/talk.wav";

/// The sound near water.
pub const WATER_SOUND: &str = "ambience/water1.wav";

/// The sound near sky.
pub const SKY_SOUND: &str = "ambience/wind2.wav";

/// Commands the game's scripts answer, and what the console passes on to them.
pub const GAME_COMMANDS: [&str; 4] = ["kill", "god", "notarget", "give"];

/// The game's rules, and what each starts as.
pub const GAME_RULES: [(&str, &str); 9] = [
    ("skill", "1"),
    ("deathmatch", "0"),
    ("coop", "0"),
    ("teamplay", "0"),
    ("fraglimit", "0"),
    ("timelimit", "0"),
    ("samelevel", "0"),
    ("noexit", "0"),
    ("registered", "1"),
];

/// Rules that change in play, rather than from the next level on.
pub const LIVE_RULES: [&str; 4] = ["skill", "teamplay", "fraglimit", "timelimit"];

/// The file of music track `track`.
#[must_use]
pub fn music_file(track: u8) -> String {
    format!("music/track{track:02}.ogg")
}

/// Reads the palette.
///
/// # Errors
///
/// Returns an error if it's missing or the wrong size.
pub fn load_palette(filesystem: &FileSystem) -> Result<Palette, Box<dyn Error>> {
    let bytes = filesystem.read(PALETTE)?;
    Ok(Palette::parse(&bytes).ok_or("gfx/palette.lmp is the wrong size")?)
}

/// A level read from the game's files.
#[derive(Debug)]
pub struct LoadedLevel {
    /// The map.
    pub map: Arc<Map>,
    /// The palette it's drawn in.
    pub palette: Palette,
    /// Where a player starts.
    pub start: SpawnPoint,
}

/// Reads the level called `name`, with its coloured light if it has any.
///
/// # Errors
///
/// Returns an error if its files are missing or can't be read.
pub fn load_level(filesystem: &FileSystem, name: &str) -> Result<LoadedLevel, Box<dyn Error>> {
    let bsp = filesystem.read(&format!("maps/{name}.bsp"))?;
    let lit = filesystem.read(&format!("maps/{name}.lit")).ok();
    let palette = load_palette(filesystem)?;
    let map = Arc::new(import_map(&bsp, lit.as_deref(), &palette)?);
    let start = spawn_point(&map);
    Ok(LoadedLevel {
        map,
        palette,
        start,
    })
}
