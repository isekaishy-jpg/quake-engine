//! Where settings and saved games live, and how they're written (proposal 0044).

use engine_core::time::TICK_SECONDS;
use std::error::Error;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use engine_console::Console;
use engine_net::loopback;
use engine_physics::CollisionWorld;
use engine_server::{SavedGame, Server};
use fixture_quake::{BASE_GAME, LoadedLevel, SCRIPTS, load_level};
use serde::{Deserialize, Serialize};

use crate::app::{CONFIG_NAME, World, player_client};
use crate::options::Options;

/// The folder under the user's data for this engine.
const PRODUCT_FOLDER: &str = "QuakeEngine";

/// The format of saved games. A save of another version isn't read.
const SAVE_VERSION: u32 = 1;

/// The width of a level's title in a save's description, from
/// `Host_SavegameComment`.
const TITLE_WIDTH: usize = 22;

/// The first line of a saved game, which says what it is without reading it all.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct SaveHeader {
    /// [`SAVE_VERSION`].
    version: u32,
    /// What the menus show.
    description: String,
}

/// Where settings and saves live: the user's own, never the game's folder.
pub(crate) fn user_directory(options: &Options) -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_DATA_HOME"))
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let game = options
        .game_directories
        .last()
        .map_or(BASE_GAME, String::as_str);
    base.join(PRODUCT_FOLDER).join(game)
}

/// The file of the save called `name`, which can't name a folder.
fn save_path(directory: &Path, name: &str) -> Result<PathBuf, Box<dyn Error>> {
    let plain = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !plain {
        return Err(format!("\"{name}\" isn't a save's name").into());
    }
    Ok(directory.join("saves").join(format!("{name}.sav")))
}

/// What the save called `name` says it is.
pub(crate) fn describe(directory: &Path, name: &str) -> Option<String> {
    let file = File::open(save_path(directory, name).ok()?).ok()?;
    let mut line = String::new();
    BufReader::new(file).read_line(&mut line).ok()?;
    let header: SaveHeader = serde_json::from_str(&line).ok()?;
    (header.version == SAVE_VERSION).then_some(header.description)
}

/// What the menus show for the game being played.
fn description(world: &World) -> String {
    let title: String = world.hud.level_title().chars().take(TITLE_WIDTH).collect();
    let (killed, total) = world.hud.monsters();
    format!("{title:<TITLE_WIDTH$}kills:{killed:3}/{total:3}")
}

/// Saves the game as `name`, from `Host_Savegame_f`.
pub(crate) fn save(world: &World, name: &str) -> Result<(), Box<dyn Error>> {
    let path = save_path(&world.user_directory, name)?;
    if !world.can_save() {
        return Err("can't save now".into());
    }
    let server = world.server.as_ref().ok_or("no game to save")?;
    let header = SaveHeader {
        version: SAVE_VERSION,
        description: description(world),
    };
    let mut text = serde_json::to_string(&header)?;
    text.push('\n');
    text.push_str(&serde_json::to_string(&server.save()?)?);
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    // A failed write mustn't cost the save that was there.
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, text)?;
    fs::rename(&temporary, &path)?;
    Ok(())
}

/// Goes back to the save called `name`, from `Host_Loadgame_f`.
pub(crate) fn load(world: &mut World, name: &str) -> Result<(), Box<dyn Error>> {
    let path = save_path(&world.user_directory, name)?;
    let text = fs::read_to_string(&path)?;
    let (header, body) = text.split_once('\n').ok_or("not a saved game")?;
    let header: SaveHeader = serde_json::from_str(header)?;
    if header.version != SAVE_VERSION {
        return Err(format!("saved by version {}", header.version).into());
    }
    let saved: SavedGame = serde_json::from_str(body)?;
    let level = saved.level_name().to_owned();
    let LoadedLevel { map, palette, .. } = load_level(&world.filesystem, &level)?;
    let collision = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let (client_end, server_end) = loopback();
    let server = Server::resume(
        Box::new(server_end),
        Rc::clone(&collision),
        SCRIPTS,
        TICK_SECONDS,
        saved,
    )?;
    let origin = server.player_origin();
    world.leave_game();
    world.level = server.level().clone();
    // From `Host_Loadgame_f`: the skill is the game's, not this session's.
    let skill = world.level.settings.iter().find(|rule| rule.0 == "skill");
    if let Some((_, skill)) = skill {
        world.commands.push(format!("skill {skill}"));
    }
    world.server = Some(server);
    world.set_collision(Rc::clone(&collision));
    world.client = Some(player_client(Box::new(client_end), collision, origin));
    world.after_load(&map, palette)
}

/// Writes the bindings, aliases and saved cvars, from `Host_WriteConfiguration`.
pub(crate) fn write_config(console: &Console<World>, world: &World) {
    let mut text = String::from("// Written by the engine. Edits are overwritten.\n");
    text.push_str("unbindall\n");
    for (key, command) in world.bindings.iter() {
        let _ignored = writeln!(text, "bind \"{key}\" \"{command}\"");
    }
    for (name, value) in console.aliases() {
        let _ignored = writeln!(text, "alias {name} \"{value}\"");
    }
    for (name, value) in console.saved_cvars() {
        let _ignored = writeln!(text, "{name} \"{value}\"");
    }
    let _ignored = fs::write(world.user_directory.join(CONFIG_NAME), text);
}

/// The next free name for a screenshot.
pub(crate) fn screenshot_path(directory: &Path) -> String {
    let folder = directory.join("screenshots");
    let _ignored = fs::create_dir_all(&folder);
    (0..10000)
        .map(|number| folder.join(format!("quake{number:04}.png")))
        .find(|path| !path.exists())
        .unwrap_or_else(|| folder.join("quake.png"))
        .display()
        .to_string()
}
