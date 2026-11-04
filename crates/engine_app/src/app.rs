//! The running engine: the main loop's two halves.

use std::io::{BufRead, Write};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use engine_assets::{AssetError, FileSystem};
use engine_console::{Console, CvarFlags};
use engine_core::time::{TickClock, TickRate};
use engine_platform::{AppError, Frame, FrameOutcome, InputEvent, PlatformApp, ServiceOutcome};
use fixture_quake::mount_game;

use crate::commands;
use crate::options::Options;

/// The simulation rate (proposal 0010).
const TICK_RATE: Option<TickRate> = TickRate::new(72);

/// The most time a stall can catch up on.
const MAX_CATCH_UP: Duration = Duration::from_millis(250);

/// The state console commands work on.
pub(crate) struct World {
    /// The search path.
    pub(crate) filesystem: FileSystem,
    /// Set by `quit`.
    pub(crate) quit: bool,
    /// Commands from the command line, until `stuffcmds` takes them.
    pub(crate) startup_commands: String,
    /// Ticks run so far.
    pub(crate) ticks: u64,
    /// Frames shown so far.
    pub(crate) frames: u64,
    /// Time since the host started.
    pub(crate) run_time: Duration,
}

/// The engine: everything the main loop drives.
pub(crate) struct Engine {
    /// The console.
    console: Console<World>,
    /// The console's context.
    world: World,
    /// Turns real time into ticks.
    clock: TickClock,
    /// When the clock last advanced.
    last_service: Option<Duration>,
    /// Lines typed on standard input.
    typed: Option<Receiver<String>>,
}

/// Reads standard input on its own thread, because reading blocks. The thread
/// ends with the process, since a blocked read can't be interrupted.
fn read_standard_input() -> Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    let _detached = std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            // Windows PowerShell starts piped text with a byte order mark.
            let line = line.trim_start_matches('\u{feff}').to_owned();
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    receiver
}

impl Engine {
    /// Builds the engine from the command line.
    pub(crate) fn new(options: Options) -> Result<Self, AssetError> {
        let mut filesystem = FileSystem::new();
        for game in &options.game_directories {
            mount_game(&mut filesystem, &options.base_directory.join(game))?;
        }
        let mut console = Console::new();
        commands::register(&mut console);
        console.register_cvar("show_input", "0", CvarFlags::default());
        let rate = TICK_RATE.expect("the tick rate is a non-zero constant");
        Ok(Self {
            console,
            world: World {
                filesystem,
                quit: false,
                startup_commands: options.startup_commands,
                ticks: 0,
                frames: 0,
                run_time: Duration::ZERO,
            },
            clock: TickClock::new(rate, MAX_CATCH_UP),
            last_service: None,
            typed: None,
        })
    }

    /// Runs one simulation tick.
    fn tick(&mut self) {
        self.world.ticks += 1;
        if let Some(typed) = &self.typed {
            for line in typed.try_iter() {
                self.console.add(&line);
            }
        }
        self.console.execute(&mut self.world);
        self.show_output();
    }

    /// Writes waiting console lines to standard output. This is the reporting
    /// layer until the console is drawn. A closed output isn't a reason to stop.
    fn show_output(&mut self) {
        let lines = self.console.take_output();
        if lines.is_empty() {
            return;
        }
        let mut output = std::io::stdout().lock();
        for line in lines {
            let _ignored = writeln!(output, "{line}");
        }
        let _ignored = output.flush();
    }

    /// Shows input when the `show_input` cvar is on.
    fn show_input(&mut self, events: &[InputEvent]) {
        let enabled = self
            .console
            .cvar("show_input")
            .is_some_and(|cvar| cvar.number() != 0.0);
        if enabled {
            for event in events {
                self.console.print(format!("{event:?}"));
            }
        }
    }
}

impl PlatformApp for Engine {
    fn started(&mut self) -> Result<(), AppError> {
        self.typed = Some(read_standard_input());
        self.console.add("exec quake.rc");
        Ok(())
    }

    fn service(&mut self, now: Duration) -> ServiceOutcome {
        self.world.run_time = now;
        let elapsed = now.saturating_sub(self.last_service.unwrap_or(now));
        self.last_service = Some(now);
        for _ in 0..self.clock.advance(elapsed) {
            self.tick();
        }
        if self.world.quit {
            ServiceOutcome::Stop
        } else {
            ServiceOutcome::After(self.clock.until_next())
        }
    }

    fn frame(&mut self, frame: &Frame) -> FrameOutcome {
        self.world.frames += 1;
        self.show_input(&frame.input);
        if self.world.quit {
            FrameOutcome::Stop
        } else {
            FrameOutcome::Continue
        }
    }

    fn stopping(&mut self) {
        self.show_output();
    }
}
