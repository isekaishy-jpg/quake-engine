//! The running engine: the main loop's two halves.

use std::io::{BufRead, Write};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use engine_assets::{AssetError, FileSystem};
use engine_audio::{Audio, Listener};
use engine_client::{
    AmbientSounds, Bindings, Buttons, Client, Look, LookScale, StepSmoother, VIEW_HEIGHT, view_bob,
};
use engine_console::{Console, CvarFlags};
use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_core::time::{FpsMeter, TickClock, TickRate};
use engine_map::Map;
use engine_net::loopback;
use engine_physics::CollisionWorld;
use engine_platform::{
    AppError, Frame, FrameOutcome, InputEvent, MouseButton, PlatformApp, ServiceOutcome, Window,
};
use engine_render::{EntityDraw, Renderer, Target, View};
use engine_server::Server;
use fixture_quake::{Palette, SCRIPTS, import_map, mount_game, mount_music, spawn_point};

use crate::commands;
use crate::models::ModelBank;
use crate::options::Options;
use crate::sounds::SoundBank;

/// The simulation rate (proposal 0010).
const TICK_RATE: Option<TickRate> = TickRate::new(72);

/// The length of a tick.
const TICK_SECONDS: f64 = 1.0 / 72.0;

/// The most time a stall can catch up on.
const MAX_CATCH_UP: Duration = Duration::from_millis(250);

/// Modern movement bindings, set after the user's config (proposal 0019).
const MOVE_BINDINGS: &str = "bind w +forward; bind s +back; bind a +moveleft; bind d +moveright; \
                             bind space +jump; bind c +movedown; bind shift +speed";

/// The sound levels the console sets.
#[derive(Clone, Copy, Debug, PartialEq)]
struct AudioLevels {
    /// `volume`.
    volume: f32,
    /// `bgmvolume`.
    music: f32,
    /// `ambient_level`.
    ambient: f32,
    /// `ambient_water`.
    water: f32,
}

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
    /// How many of those were in reduced mode.
    pub(crate) reduced_frames: u64,
    /// Time since the host started.
    pub(crate) run_time: Duration,
    /// The view.
    pub(crate) look: Look,
    /// Raw motion events received so far.
    pub(crate) motion_events: u64,
    /// The renderer, once the window exists.
    pub(crate) renderer: Option<Renderer>,
    /// The local player's side of the game, once a map is loaded.
    pub(crate) client: Option<Client>,
    /// The game, once a map is loaded.
    pub(crate) server: Option<Server>,
    /// Smooths the view on stairs.
    step: StepSmoother,
    /// The movement buttons held.
    pub(crate) buttons: Buttons,
    /// Key bindings.
    pub(crate) bindings: Bindings,
    /// Seconds since the map loaded.
    pub(crate) map_time: f64,
    /// The field of view in degrees, at 4:3.
    pub(crate) fov: f32,
    /// Everything that sounds.
    pub(crate) audio: Audio,
    /// Sounds loaded so far.
    pub(crate) sounds: SoundBank,
    /// The music track playing.
    pub(crate) music_track: Option<u8>,
    /// Where sounds are heard from.
    listener: Listener,
    /// The levels and mute last given to `audio`.
    audio_settings: Option<(AudioLevels, bool)>,
    /// How much water and sky can be heard.
    ambience: Option<AmbientSounds>,
    /// Models loaded so far.
    pub(crate) models: ModelBank,
    /// The map, for the light on models.
    map: Option<Arc<Map>>,
}

impl World {
    /// Loads the map called `name` and moves to its spawn point.
    pub(crate) fn load_map(&mut self, name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let bsp = self.filesystem.read(&format!("maps/{name}.bsp"))?;
        let lit = self.filesystem.read(&format!("maps/{name}.lit")).ok();
        let palette = self.filesystem.read("gfx/palette.lmp")?;
        let palette = Palette::parse(&palette).ok_or("gfx/palette.lmp is the wrong size")?;
        let map = Arc::new(import_map(&bsp, lit.as_deref(), &palette)?);
        let start = spawn_point(&map);
        let collision = Rc::new(CollisionWorld::new(Arc::clone(&map)));
        let (client_end, server_end) = loopback();
        self.server = Some(Server::new(
            Box::new(server_end),
            Rc::clone(&collision),
            &map,
            start.origin,
            SCRIPTS,
            TICK_SECONDS,
        )?);
        self.client = Some(Client::new(Box::new(client_end), collision, start.origin));
        self.step = StepSmoother::default();
        self.look.set_view(f64::from(start.yaw), 0.0);
        self.map_time = 0.0;
        self.audio.stop_sounds();
        let water = self
            .sounds
            .clip("ambience/water1.wav", &self.filesystem, &mut self.audio);
        let sky = self
            .sounds
            .clip("ambience/wind2.wav", &self.filesystem, &mut self.audio);
        self.audio.set_ambient_clips(water, sky);
        self.ambience = Some(AmbientSounds::new(Arc::clone(&map)));
        self.models.clear_numbers();
        self.map = Some(Arc::clone(&map));
        if let Some(renderer) = &mut self.renderer {
            renderer.load_map(map)?;
        }
        Ok(())
    }

    /// A command for the current input.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "f32 keeps angles to a few thousandths of a degree"
    )]
    fn command(&self) -> UserCommand {
        self.buttons
            .command(self.look.yaw() as f32, self.look.pitch() as f32)
    }

    /// Runs one tick of the game.
    fn run_tick(&mut self, seconds: f32) {
        let command = self.command();
        if let (Some(client), Some(server)) = (&mut self.client, &mut self.server) {
            client.send_command(command, seconds);
            server.tick();
            client.receive();
            if let Some((yaw, pitch)) = client.take_view() {
                self.look.set_view(f64::from(yaw), f64::from(pitch));
            }
        }
        self.play_sounds();
    }

    /// Passes on the sounds and music the client has heard about.
    fn play_sounds(&mut self) {
        let Some(client) = &mut self.client else {
            return;
        };
        for (number, name) in client.take_model_names() {
            self.models.name_number(
                u32::from(number),
                &name,
                &self.filesystem,
                self.renderer.as_mut(),
            );
        }
        let sounds = client.take_sounds();
        let static_sounds = client.take_static_sounds();
        let music = client.take_music();
        for sound in sounds {
            let clip = self
                .sounds
                .clip(&sound.name, &self.filesystem, &mut self.audio);
            if let Some(clip) = clip {
                self.audio.start(&sound, clip);
            }
        }
        for sound in static_sounds {
            let clip = self
                .sounds
                .clip(&sound.name, &self.filesystem, &mut self.audio);
            if let Some(clip) = clip {
                self.audio.add_static(&sound, clip);
            }
        }
        if let Some(track) = music {
            self.play_track(track);
        }
    }

    /// Plays music track `track`, unless it's already playing. With no file
    /// for it there's no music, as with no disc in the drive.
    pub(crate) fn play_track(&mut self, track: u8) {
        if self.music_track == Some(track) {
            return;
        }
        self.music_track = Some(track);
        let bytes = self.filesystem.read(&format!("music/track{track:02}.ogg"));
        let playing = bytes.is_ok_and(|bytes| self.audio.play_music(bytes).is_ok());
        if !playing {
            self.audio.stop_music();
        }
    }

    /// Moves sound on by `seconds`, at these volumes.
    fn update_audio(&mut self, seconds: f64, levels: AudioLevels, muted: bool) {
        if self.audio_settings != Some((levels, muted)) {
            self.audio_settings = Some((levels, muted));
            self.audio.set_volume(levels.volume);
            self.audio.set_music_volume(levels.music);
            self.audio.set_ambient_level(levels.ambient);
            self.audio.set_water_share(levels.water);
            self.audio.set_muted(muted);
        }
        let origin = self.listener.origin;
        let levels = self
            .ambience
            .as_mut()
            .map_or([0, 0], |ambience| ambience.levels(origin));
        self.audio.update(self.listener, seconds, levels);
    }

    /// Text from the server, and a level to change to.
    fn take_server_news(&mut self) -> (Vec<String>, Option<String>) {
        let prints = self.client.as_mut().map(Client::take_prints);
        let level = self.server.as_mut().and_then(Server::take_next_level);
        (prints.unwrap_or_default(), level)
    }

    /// The view, `leftover` seconds past the last tick. `frame_seconds` is
    /// the time since the last view, for smoothing.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "angles and map time need no more than f32 for drawing"
    )]
    pub(crate) fn view(
        &mut self,
        leftover: f32,
        fraction: f32,
        frame_seconds: f32,
    ) -> (View, Vec<ModelInstance>, Vec<EntityDraw>) {
        let command = self.command();
        let mut origin = Vec3::ZERO;
        let mut instances = Vec::new();
        let mut entities = Vec::new();
        let mut styles = [1.0; engine_client::LIGHT_STYLE_COUNT];
        if let Some(client) = &mut self.client {
            let state = client.render_state(&command, leftover);
            let body = state.origin + client.ground_lag(&state, fraction);
            let step = self.step.update(body.z, state.on_ground, frame_seconds);
            let speed = Vec3::new(state.velocity.x, state.velocity.y, 0.0).length();
            let time = self.map_time as f32;
            let bob = view_bob(time, speed);
            origin = body + Vec3::new(0.0, 0.0, VIEW_HEIGHT + step + bob);
            let (sin, cos) = self.look.yaw().to_radians().sin_cos();
            self.listener = Listener {
                origin,
                right: Vec3::new(sin as f32, -cos as f32, 0.0),
            };
            instances = client.draw_instances(fraction);
            styles = client.light_styles(self.map_time);
            if let Some(map) = &self.map {
                entities.extend(
                    client
                        .draw_entities(fraction)
                        .iter()
                        .filter_map(|entity| self.models.draw(entity, map, &styles, time)),
                );
                let (model, frame) = client.weapon();
                let yaw = self.look.yaw() as f32;
                let pitch = self.look.pitch() as f32;
                let (yaw_sin, yaw_cos) = yaw.to_radians().sin_cos();
                let (pitch_sin, pitch_cos) = pitch.to_radians().sin_cos();
                let forward = Vec3::new(yaw_cos * pitch_cos, yaw_sin * pitch_cos, -pitch_sin);
                // From `V_CalcRefdef`, with the status bar's 2 units.
                let placed = origin + forward * (bob * 0.4) + Vec3::new(0.0, 0.0, 2.0);
                let light = map.light_at(state.origin, &styles);
                let angles = Vec3::new(-pitch, yaw, 0.0);
                entities.extend(self.models.draw_weapon(
                    u32::from(model),
                    frame,
                    (placed, angles),
                    light,
                    time,
                ));
            }
        }
        let view = View {
            origin,
            yaw: self.look.yaw() as f32,
            pitch: self.look.pitch() as f32,
            fov: self.fov,
            time: self.map_time as f32,
            styles,
        };
        (view, instances, entities)
    }
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
    /// The window's size, for the renderer's first swapchain.
    size: (u32, u32),
    /// The window lease. It must outlive the renderer.
    window: Option<Window>,
    /// The frame rate shown on screen.
    fps: FpsMeter,
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
    pub(crate) fn new(options: Options, size: (u32, u32)) -> Result<Self, AssetError> {
        let mut filesystem = FileSystem::new();
        for game in &options.game_directories {
            mount_music(&mut filesystem, &options.base_directory, game);
        }
        for game in &options.game_directories {
            mount_game(&mut filesystem, &options.base_directory.join(game))?;
        }
        let mut console = Console::new();
        commands::register(&mut console);
        console.register_cvar("show_input", "0", CvarFlags::default());
        let saved = CvarFlags {
            saved: true,
            server: false,
        };
        console.register_cvar("m_capture", "0", CvarFlags::default());
        console.register_cvar("sensitivity", "3", saved);
        console.register_cvar("m_yaw", "0.022", saved);
        console.register_cvar("m_pitch", "0.022", saved);
        console.register_cvar("fov", "90", saved);
        console.register_cvar("show_fps", "1", saved);
        console.register_cvar("vid_vsync", "0", saved);
        console.register_cvar("volume", "0.7", saved);
        console.register_cvar("bgmvolume", "1", saved);
        console.register_cvar("ambient_level", "0.3", saved);
        console.register_cvar("ambient_water", "0.5", saved);
        let rate = TICK_RATE.expect("the tick rate is a non-zero constant");
        Ok(Self {
            console,
            world: World {
                filesystem,
                quit: false,
                startup_commands: options.startup_commands,
                ticks: 0,
                frames: 0,
                reduced_frames: 0,
                run_time: Duration::ZERO,
                look: Look::new(LookScale::from_settings(0.0, 0.0, 0.0)),
                motion_events: 0,
                renderer: None,
                client: None,
                server: None,
                step: StepSmoother::default(),
                buttons: Buttons::default(),
                bindings: Bindings::new(),
                map_time: 0.0,
                fov: 90.0,
                audio: if options.no_sound {
                    Audio::silent()
                } else {
                    Audio::new()
                },
                sounds: SoundBank::default(),
                music_track: None,
                listener: Listener::default(),
                audio_settings: None,
                ambience: None,
                models: ModelBank::default(),
                map: None,
            },
            clock: TickClock::new(rate, MAX_CATCH_UP),
            last_service: None,
            typed: None,
            size,
            window: None,
            fps: FpsMeter::new(),
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
        self.world.look.set_scale(self.look_scale());
        let vsync = self.cvar_number("vid_vsync") != 0.0;
        if let Some(renderer) = &mut self.world.renderer {
            renderer.set_vsync(vsync);
        }
        self.world.fov = self
            .console
            .cvar("fov")
            .map_or(90.0, engine_console::Cvar::number);
        let seconds = 1.0 / f64::from(self.clock.rate().hertz());
        self.world.map_time += seconds;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a tick's length needs no more than f32"
        )]
        self.world.run_tick(seconds as f32);
        let (prints, level) = self.world.take_server_news();
        for text in prints {
            self.console.print(text);
        }
        if let Some(level) = level {
            match self.world.load_map(&level) {
                Ok(()) => self.console.print(format!("loaded {level}")),
                Err(error) => self
                    .console
                    .print(format!("couldn't load {level}: {error}")),
            }
        }
        self.show_output();
    }

    /// Runs what a key is bound to. Escape is reserved until there's a menu.
    fn key(&mut self, name: &str, pressed: bool) {
        if name == "ESCAPE" {
            if pressed {
                let captured = self.cvar_number("m_capture") != 0.0;
                self.console.add(if captured {
                    "m_capture 0"
                } else {
                    "m_capture 1"
                });
            }
            return;
        }
        if pressed {
            if let Some(command) = self.world.bindings.press(name) {
                self.console.add(command);
            }
        } else if let Some(command) = self.world.bindings.release(name) {
            self.console.add(&command);
        }
    }

    /// The look scale from the cvars.
    fn look_scale(&self) -> LookScale {
        LookScale::from_settings(
            self.cvar_number("sensitivity"),
            self.cvar_number("m_yaw"),
            self.cvar_number("m_pitch"),
        )
    }

    /// The value of a cvar registered at startup.
    fn cvar_number(&self, name: &str) -> f64 {
        self.console
            .cvar(name)
            .map_or(0.0, |cvar| f64::from(cvar.number()))
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
}

impl PlatformApp for Engine {
    fn started(&mut self, window: Window) -> Result<(), AppError> {
        let target = Target::Window(window.native()?);
        self.window = Some(window);
        self.world.renderer = Some(Renderer::new(target, self.size)?);
        self.world.look.set_scale(self.look_scale());
        self.typed = Some(read_standard_input());
        self.console.add("exec quake.rc");
        self.console.add(MOVE_BINDINGS);
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

    fn input(&mut self, event: &InputEvent, time: Duration) {
        match *event {
            InputEvent::Motion { x, y } => {
                self.world.look.motion(x, y);
                self.world.motion_events += 1;
            }
            InputEvent::Key {
                name,
                pressed,
                repeat: false,
            } => self.key(name, pressed),
            InputEvent::Button {
                button: MouseButton::Left,
                pressed: true,
            } if self.cvar_number("m_capture") == 0.0 => self.console.add("m_capture 1"),
            InputEvent::Button { button, pressed } => {
                let name = match button {
                    MouseButton::Left => "MOUSE1",
                    MouseButton::Right => "MOUSE2",
                    MouseButton::Middle => "MOUSE3",
                };
                self.key(name, pressed);
            }
            InputEvent::FocusLost => self.world.buttons = Buttons::default(),
            InputEvent::Resized { width, height } => {
                if let Some(renderer) = &mut self.world.renderer {
                    renderer.resize(width, height);
                }
            }
            _ => {}
        }
        if self.cvar_number("show_input") != 0.0 && !matches!(event, InputEvent::Motion { .. }) {
            self.console
                .print(format!("{:.3} {event:?}", time.as_secs_f64()));
        }
    }

    fn frame(&mut self, frame: &Frame) -> FrameOutcome {
        self.world.frames += 1;
        self.world.reduced_frames += u64::from(frame.reduced);
        if frame.reduced {
            self.fps.reset();
        } else if let Some(rate) = self.fps.frame(frame.delta) {
            let text = if self.cvar_number("show_fps") != 0.0 {
                format!("{rate} FPS")
            } else {
                String::new()
            };
            if let Some(renderer) = &mut self.world.renderer {
                renderer.set_overlay_text(&text);
            }
        }
        if !frame.reduced {
            let since_service = frame
                .now
                .saturating_sub(self.last_service.unwrap_or(frame.now));
            let tick_seconds = 1.0 / f64::from(self.clock.rate().hertz());
            let fraction = self.clock.fraction_after(since_service);
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a frame's timing needs no more than f32"
            )]
            let (view, instances, entities) = self.world.view(
                (fraction * tick_seconds) as f32,
                fraction as f32,
                frame.delta.as_secs_f64() as f32,
            );
            if let Some(renderer) = &mut self.world.renderer
                && let Err(error) = renderer.draw(&view, &instances, &entities)
            {
                self.console.print(format!("renderer: {error}"));
                self.show_output();
                return FrameOutcome::Stop;
            }
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "volumes need no more than f32"
        )]
        self.world.update_audio(
            frame.delta.as_secs_f64(),
            AudioLevels {
                volume: self.cvar_number("volume") as f32,
                music: self.cvar_number("bgmvolume") as f32,
                ambient: self.cvar_number("ambient_level") as f32,
                water: self.cvar_number("ambient_water") as f32,
            },
            frame.reduced,
        );
        if self.world.quit {
            FrameOutcome::Stop
        } else {
            FrameOutcome::Continue
        }
    }

    fn mouse_capture(&self) -> bool {
        self.cvar_number("m_capture") != 0.0
    }

    fn notice(&mut self, message: String) {
        self.console.print(message);
    }

    fn stopping(&mut self) {
        self.show_output();
        self.world.renderer = None;
        self.window = None;
    }
}
