//! Running the real game from the tools, so a map can be seen and played.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::session::Session;

/// How long the game gets to start.
const START_WAIT: Duration = Duration::from_millis(2500);

/// How long a map gets to load.
const LOAD_WAIT: Duration = Duration::from_millis(2500);

/// The game's program, which is built beside the tools.
const GAME: &str = if cfg!(windows) {
    "quake-engine.exe"
} else {
    "quake-engine"
};

/// Where the game's data usually is, for whoever hasn't said.
const USUAL_DATA: [&str; 2] = [
    "C:/Program Files (x86)/Steam/steamapps/common/Quake",
    "C:/Program Files/Steam/steamapps/common/Quake",
];

/// The first of `places` that has the game's data in it.
#[must_use]
pub fn data_in<P: AsRef<Path>>(places: &[P]) -> Option<PathBuf> {
    places
        .iter()
        .map(AsRef::as_ref)
        .find(|place| place.join("id1").join("pak0.pak").is_file())
        .map(Path::to_owned)
}

/// The game beside the program that's running, if it's there.
fn game_beside_this_program() -> Option<PathBuf> {
    let game = std::env::current_exe().ok()?.parent()?.join(GAME);
    game.is_file().then_some(game)
}

/// What's needed to run the game.
#[derive(Clone, Debug)]
pub struct GameRunner {
    /// The game's program.
    program: PathBuf,
    /// The game data directory.
    base: PathBuf,
    /// The content directory.
    content: PathBuf,
    /// The map's name.
    map: String,
}

impl GameRunner {
    /// What runs `map` from `content`, with `program` and the data in `base`.
    #[must_use]
    pub fn new(program: PathBuf, base: PathBuf, content: PathBuf, map: String) -> Self {
        Self {
            program,
            base,
            content,
            map,
        }
    }

    /// Finds the game for a saved map. `ENGINE_GAME` and `ENGINE_BASEDIR` say where,
    /// and without them the game beside this program and the usual data are used, so
    /// nobody has to set anything to play what they made.
    ///
    /// # Errors
    ///
    /// Returns a message, in plain words, if there's no game, no data, or no saved map.
    pub fn from_environment(session: &Session) -> Result<Self, String> {
        let program = std::env::var_os("ENGINE_GAME")
            .map(PathBuf::from)
            .or_else(game_beside_this_program)
            .ok_or("Can't find the game. Build it, or set ENGINE_GAME.")?;
        let base = std::env::var_os("ENGINE_BASEDIR")
            .map(PathBuf::from)
            .or_else(|| data_in(&USUAL_DATA))
            .ok_or("Can't find the game's data. Install it, or set ENGINE_BASEDIR.")?;
        let path = session.path.as_ref().ok_or("save the map first")?;
        let map = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("the map has no name")?
            .to_owned();
        Ok(Self::new(program, base, session.content.clone(), map))
    }

    /// The command that opens the game on the map for someone to play, with sound.
    #[must_use]
    pub fn play_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        let _command = command
            .arg("-basedir")
            .arg(&self.base)
            .arg("-content")
            .arg(&self.content)
            .arg("-window")
            .args(["+map", &self.map])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        command
    }

    /// `play`: opens the game on the map and leaves it for someone to play. It stays
    /// open until they close it, and this returns at once.
    ///
    /// # Errors
    ///
    /// Returns a message if the game can't start.
    pub fn play(&self) -> Result<Json, String> {
        let mut child = self
            .play_command()
            .spawn()
            .map_err(|error| format!("couldn't start the game: {error}"))?;
        let _waiter = std::thread::spawn(move || child.wait());
        Ok(json!({ "map": self.map }))
    }

    /// Starts the game, windowed and silent, loaded on the map.
    fn start(&self) -> Result<Child, String> {
        let mut child = Command::new(&self.program)
            .arg("-basedir")
            .arg(&self.base)
            .arg("-content")
            .arg(&self.content)
            .args(["-window", "-nosound"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("couldn't start the game: {error}"))?;
        std::thread::sleep(START_WAIT);
        Self::say(&mut child, &format!("map {}", self.map))?;
        std::thread::sleep(LOAD_WAIT);
        Ok(child)
    }

    /// Types a console line.
    fn say(child: &mut Child, line: &str) -> Result<(), String> {
        let stdin = child.stdin.as_mut().ok_or("the game has no input")?;
        writeln!(stdin, "{line}").map_err(|error| format!("the game went away: {error}"))?;
        stdin.flush().map_err(|error| error.to_string())
    }

    /// Ends the game.
    fn finish(mut child: Child) {
        let _quit = Self::say(&mut child, "quit");
        std::thread::sleep(Duration::from_millis(800));
        let _killed = child.kill();
        let _waited = child.wait();
    }

    /// `shot`: a screenshot of the map, after any console lines.
    ///
    /// # Errors
    ///
    /// Returns a message if the game can't run.
    pub fn shot(&self, args: &Json) -> Result<Json, String> {
        let path = args
            .get("path")
            .and_then(Json::as_str)
            .ok_or("path is required")?;
        let mut child = self.start()?;
        if let Some(lines) = args.get("commands").and_then(Json::as_array) {
            for line in lines.iter().filter_map(Json::as_str) {
                Self::say(&mut child, line)?;
                std::thread::sleep(Duration::from_millis(300));
            }
        }
        Self::say(
            &mut child,
            &format!("screenshot {}", path.replace('\\', "/")),
        )?;
        std::thread::sleep(Duration::from_secs(1));
        Self::finish(child);
        Ok(json!({ "path": path }))
    }

    /// `playtest`: runs the map for a while and reports whether it stayed up.
    ///
    /// # Errors
    ///
    /// Returns a message if the game can't run.
    pub fn playtest(&self, args: &Json) -> Result<Json, String> {
        let seconds = args.get("seconds").and_then(Json::as_f64).unwrap_or(5.0);
        let mut child = self.start()?;
        std::thread::sleep(Duration::from_secs_f64(seconds.clamp(0.0, 600.0)));
        let exited = child.try_wait().map_err(|error| error.to_string())?;
        let running = exited.is_none();
        Self::finish(child);
        Ok(json!({ "stayed_up": running, "seconds": seconds }))
    }
}
