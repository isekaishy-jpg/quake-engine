//! The executable and composition root. It is the only crate that knows every
//! system. See proposal 0008.

mod app;
mod commands;
mod options;

use std::io::Write;
use std::process::ExitCode;

use app::Engine;
use engine_platform::{WindowConfig, WindowMode, run};
use options::Options;

/// The window title.
const TITLE: &str = "Quake Engine";

/// The most frames per second, for a present mode that doesn't wait.
const FRAME_CAP: u32 = 240;

/// The window's first width.
const WIDTH: u32 = 1280;

/// The window's first height.
const HEIGHT: u32 = 720;

/// Frames per second while minimized, enough for the system's previews.
const REDUCED_FRAME_RATE: u32 = 10;

/// Starts the engine and reports a failure on standard error.
fn main() -> ExitCode {
    match start() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ignored = writeln!(std::io::stderr(), "quake-engine: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Reads the command line and runs the engine until it stops.
fn start() -> Result<(), Box<dyn std::error::Error>> {
    let options = Options::from_arguments(std::env::args().skip(1))?;
    let mode = if options.windowed {
        WindowMode::Windowed
    } else {
        WindowMode::Borderless
    };
    let mut engine = Engine::new(options, (WIDTH, HEIGHT))?;
    let config = WindowConfig {
        title: TITLE.to_owned(),
        width: WIDTH,
        height: HEIGHT,
        mode,
        frame_cap: Some(FRAME_CAP),
        reduced_frame_rate: REDUCED_FRAME_RATE,
    };
    run(config, &mut engine)?;
    Ok(())
}
