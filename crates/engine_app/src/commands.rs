//! The application's own console commands.

use std::error::Error;
use std::fs::File;
use std::io::BufWriter;

use engine_assets::AssetError;
use engine_console::{Args, Console};
use engine_render::Capture;

use crate::app::World;

/// The movement buttons, as the console sees them.
const BUTTONS: [&str; 8] = [
    "forward",
    "back",
    "moveleft",
    "moveright",
    "moveup",
    "movedown",
    "speed",
    "jump",
];

/// The width of a screenshot.
const SCREENSHOT_WIDTH: u32 = 1280;

/// The height of a screenshot.
const SCREENSHOT_HEIGHT: u32 = 720;

/// Registers the commands.
pub(crate) fn register(console: &mut Console<World>) {
    console.register("quit", quit);
    console.register("path", path);
    console.register("dir", dir);
    console.register("exec", exec);
    console.register("stuffcmds", stuffcmds);
    console.register("stats", stats);
    console.register("map", map);
    console.register("bind", bind);
    console.register("unbind", unbind);
    console.register("unbindall", unbindall);
    console.register("screenshot", screenshot);
    console.register("setpos", setpos);
    console.register("noclip", noclip);
    for button in BUTTONS {
        console.register(&format!("+{button}"), button_down);
        console.register(&format!("-{button}"), button_up);
    }
}

/// `quit`: stops the engine.
fn quit(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    world.quit = true;
}

/// `path`: shows the search path, highest priority first.
fn path(console: &mut Console<World>, world: &mut World, _args: &Args) {
    console.print("Current search path:");
    for label in world.filesystem.search_path() {
        console.print(label);
    }
}

/// `dir [prefix]`: lists files on the search path.
fn dir(console: &mut Console<World>, world: &mut World, args: &Args) {
    let files = world.filesystem.list(args.get(1));
    let count = files.len();
    for file in files {
        console.print(file);
    }
    console.print(format!("{count} files"));
}

/// `exec file`: runs a script next.
fn exec(console: &mut Console<World>, world: &mut World, args: &Args) {
    let name = args.get(1);
    match world.filesystem.read(name) {
        Ok(bytes) => {
            console.print(format!("execing {name}"));
            console.insert(&String::from_utf8_lossy(&bytes));
        }
        Err(AssetError::NotFound(_)) => console.print(format!("couldn't exec {name}")),
        Err(error) => console.print(format!("couldn't exec {name}: {error}")),
    }
}

/// `stuffcmds`: runs the `+` commands from the command line.
fn stuffcmds(console: &mut Console<World>, world: &mut World, _args: &Args) {
    let commands = std::mem::take(&mut world.startup_commands);
    console.insert(&commands);
}

/// `stats`: shows the tick and frame counts, and the view.
fn stats(console: &mut Console<World>, world: &mut World, _args: &Args) {
    let seconds = world.run_time.as_secs_f64();
    console.print(format!(
        "{} ticks and {} frames, {} reduced, in {seconds:.2} s",
        world.ticks, world.frames, world.reduced_frames
    ));
    console.print(format!(
        "{} motion events, yaw {:.4}, pitch {:.4}",
        world.motion_events,
        world.look.yaw(),
        world.look.pitch()
    ));
    let load = world.audio.peak_load().unwrap_or(0.0) * 100.0;
    let gap = world.audio.longest_gap().unwrap_or_default().as_secs_f64() * 1000.0;
    console.print(format!(
        "audio: device {}, {} playing, {} static in {} voices, {} of {} sounds missing, \
         music {:?}, thread peaked at {load:.1}%, longest gap {gap:.1} ms, ambient {:.2?}, \
         ears {:.2?}",
        world.audio.has_device(),
        world.audio.playing().len(),
        world.audio.static_count(),
        world.audio.static_volumes().len(),
        world.sounds.missing(),
        world.sounds.len(),
        world.music_track,
        world.audio.ambient_volumes(),
        world.audio.static_volumes()
    ));
    if let Some(renderer) = &world.renderer {
        let stats = renderer.statistics();
        let at = world
            .client
            .as_ref()
            .map_or_else(Default::default, |client| client.predicted().origin);
        console.print(format!(
            "{}: {} of {} triangles in {} batches, at {:.1} {:.1} {:.1}",
            renderer.device_name(),
            stats.drawn_triangles,
            stats.total_triangles,
            stats.batches,
            at.x,
            at.y,
            at.z
        ));
    }
}

/// `map name`: loads a map and starts at its first spawn point.
fn map(console: &mut Console<World>, world: &mut World, args: &Args) {
    match world.load_map(args.get(1)) {
        Ok(()) => console.print(format!("loaded {}", args.get(1))),
        Err(error) => console.print(format!("couldn't load {}: {error}", args.get(1))),
    }
}

/// `bind key [command]`: sets a binding, or shows it.
fn bind(console: &mut Console<World>, world: &mut World, args: &Args) {
    let key = args.get(1);
    match args.count() {
        0 | 1 => console.print("bind <key> [command]"),
        2 => {
            let text = world.bindings.get(key).map_or_else(
                || format!("\"{key}\" is not bound"),
                |command| format!("\"{key}\" = \"{command}\""),
            );
            console.print(text);
        }
        _ => {
            let command = (2..args.count())
                .map(|index| args.get(index))
                .collect::<Vec<_>>()
                .join(" ");
            world.bindings.bind(key, &command);
        }
    }
}

/// `unbind key`: removes a binding.
fn unbind(_console: &mut Console<World>, world: &mut World, args: &Args) {
    world.bindings.unbind(args.get(1));
}

/// `unbindall`: removes every binding.
fn unbindall(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    world.bindings.clear();
}

/// `+button`: holds a movement button.
fn button_down(_console: &mut Console<World>, world: &mut World, args: &Args) {
    let _known = world.buttons.set(args.get(0).trim_start_matches('+'), true);
}

/// `-button`: lets go of a movement button.
fn button_up(_console: &mut Console<World>, world: &mut World, args: &Args) {
    let _known = world
        .buttons
        .set(args.get(0).trim_start_matches('-'), false);
}

/// Writes `capture` to a PNG file at `path`.
fn write_png(capture: &Capture, path: &str) -> Result<(), Box<dyn Error>> {
    let file = BufWriter::new(File::create(path)?);
    let mut encoder = png::Encoder::new(file, capture.width, capture.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&capture.rgba)?;
    Ok(())
}

/// `screenshot path`: writes the current view to a PNG file.
fn screenshot(console: &mut Console<World>, world: &mut World, args: &Args) {
    let path = args.get(1);
    if path.is_empty() {
        console.print("screenshot <path>");
        return;
    }
    let (view, instances) = world.view(0.0, 1.0, 0.0);
    let Some(renderer) = &mut world.renderer else {
        console.print("no renderer");
        return;
    };
    let result = renderer
        .capture(&view, &instances, SCREENSHOT_WIDTH, SCREENSHOT_HEIGHT)
        .map_err(Into::into)
        .and_then(|capture| write_png(&capture, path));
    match result {
        Ok(()) => console.print(format!("wrote {path}")),
        Err(error) => console.print(format!("couldn't write {path}: {error}")),
    }
}

/// `setpos x y z [yaw pitch]`: moves the player, through the server.
fn setpos(console: &mut Console<World>, world: &mut World, args: &Args) {
    let number = |index: usize| args.get(index).parse::<f32>().ok();
    let (Some(x), Some(y), Some(z)) = (number(1), number(2), number(3)) else {
        console.print("setpos <x> <y> <z> [yaw pitch]");
        return;
    };
    if let Some(client) = &mut world.client {
        client.send_console(&format!("setpos {x} {y} {z}"));
    }
    if let (Some(yaw), Some(pitch)) = (number(4), number(5)) {
        world.look.set_view(f64::from(yaw), f64::from(pitch));
    }
}

/// `noclip`: flies through walls, through the server.
fn noclip(_console: &mut Console<World>, world: &mut World, _args: &Args) {
    if let Some(client) = &mut world.client {
        client.send_console("noclip");
    }
}
