//! The editor: the first tool on the development platform (proposal 0074).
//!
//! `engine-editor <content> [map file]`. It calls the same operations as the
//! command line, and also takes them as JSON lines on standard input, so an
//! agent can work on the map you're looking at.

mod app;
mod camera;
mod panels;
mod ui_bridge;

use std::path::Path;
use std::process::ExitCode;

use app::Editor;
use engine_platform::{WindowConfig, WindowMode, run};

/// The window's first width.
const WIDTH: u32 = 1440;

/// The window's first height.
const HEIGHT: u32 = 900;

/// Most frames a second.
const FRAME_CAP: u32 = 120;

/// Frames a second while minimized.
const REDUCED_FRAME_RATE: u32 = 5;

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let content = arguments.next().unwrap_or_else(|| "content".to_owned());
    let map = arguments.next();
    let mut editor = match Editor::new(Path::new(&content), map.as_deref(), (WIDTH, HEIGHT)) {
        Ok(editor) => editor,
        Err(error) => {
            let _ignored = std::io::Write::write_all(
                &mut std::io::stderr(),
                format!("engine-editor: {error}\n").as_bytes(),
            );
            return ExitCode::FAILURE;
        }
    };
    let config = WindowConfig {
        title: "Engine Editor".to_owned(),
        width: WIDTH,
        height: HEIGHT,
        mode: WindowMode::Windowed,
        frame_cap: Some(FRAME_CAP),
        reduced_frame_rate: REDUCED_FRAME_RATE,
    };
    match run(config, &mut editor) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ignored = std::io::Write::write_all(
                &mut std::io::stderr(),
                format!("engine-editor: {error}\n").as_bytes(),
            );
            ExitCode::FAILURE
        }
    }
}
