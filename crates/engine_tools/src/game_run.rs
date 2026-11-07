//! Running the real game from the tools, so a map can be seen and played.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::session::Session;

/// How long the game gets to start.
const START_WAIT: Duration = Duration::from_millis(2500);

/// How long a map gets to load.
const LOAD_WAIT: Duration = Duration::from_millis(2500);

/// What the game has said so far.
type Said = Arc<Mutex<Vec<String>>>;

/// Lines of the game's that mean something went wrong.
const TROUBLE: [&str; 5] = [
    "error",
    "couldn't",
    "no spawn function",
    "unknown",
    "out of date",
];

/// A game that's running, and what it's said.
struct Running {
    /// The game.
    child: Child,
    /// Its console.
    said: Said,
}

impl Running {
    /// Everything it's said.
    fn lines(&self) -> Vec<String> {
        self.said
            .lock()
            .map_or_else(|_| Vec::new(), |lines| lines.clone())
    }

    /// What went wrong, of what it said. A game that didn't load a level is the worst of it.
    fn report(&self, map: &str) -> (bool, Vec<String>) {
        let lines = self.lines();
        let loaded = lines
            .iter()
            .any(|line| line.trim() == format!("loaded {map}"));
        let trouble = lines
            .into_iter()
            .filter(|line| {
                let line = line.to_lowercase();
                !line.contains("autoexec") && TROUBLE.iter().any(|word| line.contains(word))
            })
            .collect();
        (loaded, trouble)
    }
}

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
        session
            .in_content(path)
            .map_err(|_| format!("{} has to be saved in the content to run", path.display()))?;
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

    /// Starts the game, windowed and silent, loaded on the map, listening to what it says.
    fn start(&self) -> Result<Running, String> {
        let mut child = Command::new(&self.program)
            .arg("-basedir")
            .arg(&self.base)
            .arg("-content")
            .arg(&self.content)
            .args(["-window", "-nosound"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("couldn't start the game: {error}"))?;
        let said = Said::default();
        if let Some(out) = child.stdout.take() {
            let lines = Arc::clone(&said);
            let _reader = std::thread::spawn(move || {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if let Ok(mut lines) = lines.lock() {
                        lines.push(line);
                    }
                }
            });
        }
        std::thread::sleep(START_WAIT);
        Self::say(&mut child, &format!("map {}", self.map))?;
        std::thread::sleep(LOAD_WAIT);
        Ok(Running { child, said })
    }

    /// Says `line` to the game's console.
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

    /// `shot`: a screenshot of the map, from a place if it's given, after any console lines.
    /// If the map didn't load it says so: a picture of a menu isn't a picture of a level.
    ///
    /// # Errors
    ///
    /// Returns a message if the game can't run, or the map didn't load.
    pub fn shot(&self, args: &Json) -> Result<Json, String> {
        let path = args
            .get("path")
            .and_then(Json::as_str)
            .ok_or("path is required")?;
        let mut game = self.start()?;
        let mut lines: Vec<String> = Vec::new();
        if let Some(from) = args.get("from").and_then(Json::as_array) {
            let place: Vec<String> = from.iter().map(ToString::to_string).collect();
            let number = |key: &str| args.get(key).and_then(Json::as_f64).unwrap_or(0.0);
            lines.push("noclip".to_owned());
            lines.push(format!(
                "setpos {} {} {}",
                place.join(" "),
                number("yaw"),
                number("pitch")
            ));
        }
        if let Some(commands) = args.get("commands").and_then(Json::as_array) {
            lines.extend(commands.iter().filter_map(Json::as_str).map(str::to_owned));
        }
        for line in &lines {
            Self::say(&mut game.child, line)?;
            std::thread::sleep(Duration::from_millis(300));
        }
        Self::say(
            &mut game.child,
            &format!("screenshot {}", path.replace('\\', "/")),
        )?;
        std::thread::sleep(Duration::from_secs(1));
        let (loaded, trouble) = game.report(&self.map);
        Self::finish(game.child);
        if !loaded {
            return Err(format!(
                "{} didn't load, so that's no picture of it: {trouble:?}",
                self.map
            ));
        }
        Ok(json!({ "path": path, "trouble": trouble }))
    }

    /// `playtest`: runs the map for a while, and says whether it loaded and stayed up, and
    /// what went wrong.
    ///
    /// # Errors
    ///
    /// Returns a message if the game can't run.
    pub fn playtest(&self, args: &Json) -> Result<Json, String> {
        let seconds = args.get("seconds").and_then(Json::as_f64).unwrap_or(5.0);
        let mut game = self.start()?;
        std::thread::sleep(Duration::from_secs_f64(seconds.clamp(0.0, 600.0)));
        let exited = game.child.try_wait().map_err(|error| error.to_string())?;
        let (loaded, trouble) = game.report(&self.map);
        let running = exited.is_none();
        Self::finish(game.child);
        Ok(json!({
            "ok": loaded && running && trouble.is_empty(),
            "loaded": loaded,
            "stayed_up": running,
            "seconds": seconds,
            "trouble": trouble,
        }))
    }
}
