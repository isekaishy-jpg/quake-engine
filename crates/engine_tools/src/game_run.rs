//! Running the real game from the tools, so a map can be seen and played.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::session::Session;

/// How long the game gets to start.
const START_WAIT: Duration = Duration::from_millis(2500);

/// How long a map gets to load.
const LOAD_WAIT: Duration = Duration::from_millis(2500);

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
    /// Reads `ENGINE_GAME` and `ENGINE_BASEDIR` for a saved map.
    ///
    /// # Errors
    ///
    /// Returns a message if they aren't set or the map isn't saved.
    pub fn from_environment(session: &Session) -> Result<Self, String> {
        let program = std::env::var_os("ENGINE_GAME").ok_or("set ENGINE_GAME to the game")?;
        let base = std::env::var_os("ENGINE_BASEDIR").ok_or("set ENGINE_BASEDIR to game data")?;
        let path = session.path.as_ref().ok_or("save the map first")?;
        let map = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or("the map has no name")?
            .to_owned();
        Ok(Self {
            program: program.into(),
            base: base.into(),
            content: session.content.clone(),
            map,
        })
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
