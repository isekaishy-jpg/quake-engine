//! The executable and composition root. It is the only crate that knows every
//! system. See proposal 0008.

mod app;
mod commands;
mod options;

use std::io::Write;
use std::process::ExitCode;

use app::Engine;
use engine_platform::{WindowConfig, run};
use options::Options;

/// The window title.
const TITLE: &str = "Quake Engine";

/// Frames per second until a renderer paces itself.
const FRAME_CAP: u32 = 120;

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
    let mut engine = Engine::new(options)?;
    let config = WindowConfig {
        title: TITLE.to_owned(),
        width: 1280,
        height: 720,
        frame_cap: Some(FRAME_CAP),
        reduced_frame_rate: REDUCED_FRAME_RATE,
    };
    run(config, &mut engine)?;
    Ok(())
}
