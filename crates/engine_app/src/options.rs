//! The command line, following Quake's conventions.

use std::fmt;
use std::path::PathBuf;

/// The game directory used when `-game` isn't given.
const BASE_GAME: &str = "id1";

/// A command line that can't be used.
#[derive(Debug)]
pub(crate) struct OptionsError(String);

impl fmt::Display for OptionsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for OptionsError {}

/// What the command line asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Options {
    /// The directory holding `id1`.
    pub(crate) base_directory: PathBuf,
    /// The game directory, always `id1` first.
    pub(crate) game_directories: Vec<String>,
    /// Console commands from `+` arguments, run by `stuffcmds`.
    pub(crate) startup_commands: String,
}

impl Options {
    /// Reads `-basedir path`, `-game name` and `+command args`.
    pub(crate) fn from_arguments(
        arguments: impl Iterator<Item = String>,
    ) -> Result<Self, OptionsError> {
        let mut base_directory = None;
        let mut game_directories = vec![BASE_GAME.to_owned()];
        let mut startup_commands = String::new();
        let mut arguments = arguments.peekable();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "-basedir" => {
                    let path = arguments
                        .next()
                        .ok_or_else(|| OptionsError("-basedir needs a path".to_owned()))?;
                    base_directory = Some(PathBuf::from(path));
                }
                "-game" => {
                    let name = arguments
                        .next()
                        .ok_or_else(|| OptionsError("-game needs a name".to_owned()))?;
                    game_directories.push(name);
                }
                _ if argument.starts_with('+') => {
                    startup_commands.push_str(&argument[1..]);
                    while let Some(next) = arguments.peek() {
                        if next.starts_with('+') || next.starts_with('-') {
                            break;
                        }
                        startup_commands.push(' ');
                        startup_commands.push_str(next);
                        let _taken = arguments.next();
                    }
                    startup_commands.push('\n');
                }
                _ => return Err(OptionsError(format!("unknown option: {argument}"))),
            }
        }
        let base_directory = base_directory.ok_or_else(|| {
            OptionsError("usage: quake-engine -basedir <Quake directory> [+command ...]".to_owned())
        })?;
        Ok(Self {
            base_directory,
            game_directories,
            startup_commands,
        })
    }
}
