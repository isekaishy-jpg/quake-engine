//! The application's own console commands.

use engine_assets::AssetError;
use engine_console::{Args, Console};

use crate::app::World;

/// Registers the commands.
pub(crate) fn register(console: &mut Console<World>) {
    console.register("quit", quit);
    console.register("path", path);
    console.register("dir", dir);
    console.register("exec", exec);
    console.register("stuffcmds", stuffcmds);
    console.register("stats", stats);
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

/// `stats`: shows the tick and frame counts.
fn stats(console: &mut Console<World>, world: &mut World, _args: &Args) {
    let seconds = world.run_time.as_secs_f64();
    console.print(format!(
        "{} ticks and {} frames in {seconds:.2} s",
        world.ticks, world.frames
    ));
    console.print(format!(
        "{} motion events, yaw {:.4}, pitch {:.4}",
        world.motion_events,
        world.look.yaw(),
        world.look.pitch()
    ));
}
