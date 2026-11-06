//! The running engine: the main loop's two halves.

mod keys;
mod net;
mod shading;
mod shots;
mod sound;
mod view;

use self::shading::{Flames, StencilGates};

use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use engine_assets::{AssetError, DirectoryArchive, FileSystem, PackArchive};
use engine_audio::{Audio, Listener};
use engine_client::{
    AmbientSounds, Bindings, Bot, Buttons, Client, Effects, Look, LookScale, ShotClock,
    StepSmoother, ViewKick, ZoneTracker,
};
use engine_console::{Console, CvarFlags};
use engine_core::canvas::TALK_MARK;
use engine_core::entity::effect_id;
use engine_core::math::Vec3;
use engine_core::player::{USUAL_GRAVITY, UserCommand};
use engine_core::time::{FpsMeter, TICK_HERTZ, TICK_SECONDS, TickClock, TickRate};
use engine_map::Map;
use engine_net::{NetServer, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_platform::{
    AppError, Frame, FrameOutcome, InputEvent, MouseButton, PlatformApp, ServiceOutcome, Window,
};
use engine_render::{PostSettings, Renderer, Target, TextureFilter, TileStreamer};
use engine_script::UiScript;
use engine_server::{FairSettings, Level, SeatInfo, Server};
use engine_shaders::built_in_shaders;
use engine_ui::ConsoleView;
use engine_ui::UiTheme;
use fixture_quake::{
    GAME_RULES, Hud, Intermission, LIVE_RULES, LoadedLevel, Menu, Palette, ROCKET_BLAST, SCRIPTS,
    SKY_SOUND, STARTUP_SCRIPT, Score, TALK_SOUND, WATER_SOUND, mount_coloured_levels, mount_game,
    mount_music,
};
use sound::AudioLevels;
use view::DEFAULT_FOV;

use crate::keys::Keys;
use crate::look::{load_theme, load_ui_script};
use crate::models::ModelBank;
use crate::options::Options;
use crate::screen::{MENU_SETTINGS, MenuSettings, PictureBank};
use crate::sounds::SoundBank;
use crate::{commands, saves};

/// The simulation rate (proposal 0010).
const TICK_RATE: Option<TickRate> = TickRate::new(TICK_HERTZ);

/// The most time a stall can catch up on.
const MAX_CATCH_UP: Duration = Duration::from_millis(250);

/// The name of the saved settings.
pub(crate) const CONFIG_NAME: &str = "config.cfg";

/// How many frames show the loading plaque before a level changes.
const PLAQUE_FRAMES: u8 = 2;

/// Modern movement bindings, set the first time (proposal 0019).
pub(crate) const MOVE_BINDINGS: &str = "bind w +forward; bind s +back; bind a +moveleft; bind d +moveright; \
                             bind space +jump; bind c +movedown; bind shift +speed";

/// How long a connection may take to open, in seconds.
const CONNECT_SECONDS: f64 = 10.0;

/// A client for the player, with what it can know of the game's rules.
pub(crate) fn player_client(
    transport: Box<dyn Transport>,
    world: Rc<CollisionWorld>,
    origin: Vec3,
) -> Client {
    let mut client = Client::new(transport, world, origin);
    client.set_blast(Some(ROCKET_BLAST));
    client
}

/// The state console commands work on.
pub(crate) struct World {
    /// The search path.
    pub(crate) filesystem: Arc<FileSystem>,
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
    /// Keeps detailed light near the view, on a map whose light streams.
    pub(crate) tile_streamer: Option<TileStreamer>,
    /// The local player's side of the game, once a map is loaded.
    pub(crate) client: Option<Client>,
    /// The game, once a map is loaded.
    pub(crate) server: Option<Server>,
    /// Smooths the view on stairs.
    step: StepSmoother,
    /// Which stencil boxes are open.
    gates: StencilGates,
    /// What small fires are doing.
    flames: Flames,
    /// How many moving lights a frame can have.
    live_light_budget: usize,
    /// The movement buttons held.
    pub(crate) buttons: Buttons,
    /// Key bindings.
    pub(crate) bindings: Bindings,
    /// Seconds since the map loaded.
    pub(crate) map_time: f64,
    /// What's to be said about the level just loaded, once the clock has
    /// started, so that it stays on the screen for as long as other lines.
    load_warning: Option<String>,
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
    /// What the client shows of what's happening.
    pub(crate) effects: Effects,
    /// The clock of the last update of effects.
    pub(crate) effects_time: f32,
    /// Which zone the view is in.
    pub(crate) zones: ZoneTracker,
    /// The engine's own look, if the content has it.
    pub(crate) theme: UiTheme,
    /// The status bar and menus, if the content has them as a script.
    pub(crate) ui_script: Option<UiScript>,
    /// What last went wrong in it, so that it's said once and not each frame.
    pub(crate) ui_error: Option<String>,
    /// What it hasn't said yet.
    pub(crate) ui_news: Vec<String>,
    /// Whether the script's menu is open.
    pub(crate) script_menu: bool,
    /// The settings the script wants to know, as they stand.
    pub(crate) console_settings: BTreeMap<String, f64>,
    /// The title of the zone just entered, and when it was.
    pub(crate) zone_title: Option<(String, f64)>,
    /// The colours particles are given in.
    palette: Option<Palette>,
    /// What the player carries into the next level.
    next_parms: Option<Vec<f64>>,
    /// The game's flags to carry into the next level.
    next_flags: f64,
    /// The level being played, with what it started with.
    pub(crate) level: Level,
    /// The map, for the light on models.
    map: Option<Arc<Map>>,
    /// The status bar and the rest of what's over the view.
    pub(crate) hud: Hud,
    /// The menus.
    pub(crate) menu: Menu,
    /// The console, as it's drawn.
    pub(crate) console_view: ConsoleView,
    /// Pictures loaded so far.
    pub(crate) pictures: PictureBank,
    /// Whether the game is paused.
    pub(crate) paused: bool,
    /// What being hit is doing to the view.
    kick: ViewKick,
    /// A level to change to, once the plaque has been seen.
    pub(crate) level_pending: Option<String>,
    /// Frames shown since `level_pending` was set.
    level_wait: u8,
    /// Console commands from the game, waiting to run.
    pub(crate) commands: Vec<String>,
    /// `ui_scale`.
    pub(crate) ui_scale: f32,
    /// `crosshair`.
    pub(crate) crosshair: bool,
    /// `gamma`.
    gamma: f32,
    /// The frame rate as it's shown, or empty.
    pub(crate) rate_text: String,
    /// The settings the menus show.
    pub(crate) menu_settings: MenuSettings,
    /// Where configs and saves go.
    pub(crate) user_directory: PathBuf,
    /// Listens for others, while hosting.
    pub(crate) net_server: Option<NetServer>,
    /// A connection being opened.
    pub(crate) connecting: Option<Box<dyn Transport>>,
    /// The address it's to, for remembering who's there.
    pub(crate) joining: Option<String>,
    /// Who this player and host are, and who they know.
    pub(crate) keys: Keys,
    /// How long that's been going, in seconds.
    connect_time: f64,
    /// Who this player is.
    pub(crate) identity: SeatInfo,
    /// When a shot of ours can next be shown.
    shot_clock: ShotClock,
    /// What [`LIVE_RULES`] last said.
    rules_seen: Option<[f64; 4]>,
    /// The gravity last put in `sv_gravity`.
    gravity_shown: f32,
    /// What `sv_gravity` last said.
    gravity_seen: f32,
    /// Whether shots show the boxes on this screen, from `sv_unlag_debug`.
    showing_boxes: bool,
    /// Bots playing here, each a client of its own.
    pub(crate) bots: Vec<Bot>,
    /// How many bots have been made, for names and seeds.
    bots_made: u32,
    /// The map's collision, for bots and for what's heard through walls.
    collision: Option<Rc<CollisionWorld>>,
    /// How long until the room round the listener is measured again.
    room_wait: f64,
}

impl World {
    /// Starts a new game on the map called `name`.
    pub(crate) fn new_game(
        &mut self,
        name: &str,
        console: &Console<Self>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.next_parms = None;
        self.next_flags = 0.0;
        self.load_map(name, console)
    }

    /// Sets what the next level starts with.
    pub(crate) fn carry(&mut self, parms: Option<Vec<f64>>, flags: f64) {
        self.next_parms = parms;
        self.next_flags = flags;
    }

    /// Whether the player sees the game itself, with nothing open over it.
    pub(crate) fn is_playing_screen(&self) -> bool {
        !self.menu.is_open() && !self.script_menu && !self.console_view.is_open()
    }

    /// Whether other people are involved, so the game can't stop for one.
    pub(crate) fn is_online(&self) -> bool {
        self.net_server.is_some()
            || !self.bots.is_empty()
            || (self.client.is_some() && self.server.is_none())
    }

    /// Notes the map's collision, for bots.
    pub(crate) fn set_collision(&mut self, collision: Rc<CollisionWorld>) {
        self.collision = Some(collision);
    }

    /// Whether the game moves on, as `_Host_Frame` decides.
    pub(crate) fn is_running(&self) -> bool {
        self.level_pending.is_none()
            && (self.is_online() || (self.is_playing_screen() && !self.paused))
    }

    /// Whether the game can be saved now, from `Host_Savegame_f`.
    pub(crate) fn can_save(&self) -> bool {
        self.server.is_some()
            && !self.is_online()
            && self.level_pending.is_none()
            && self.hud.intermission() == Intermission::None
            && !self.hud.is_dead()
    }

    /// Loads the map called `name` and moves to its spawn point, carrying what
    /// `next_parms` and `next_flags` hold.
    pub(crate) fn load_map(
        &mut self,
        name: &str,
        console: &Console<Self>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let LoadedLevel {
            map,
            palette,
            start,
        } = crate::level::load_level(&self.filesystem, name)?;
        let level = Level {
            name: name.to_owned(),
            parms: self.next_parms.take(),
            flags: self.next_flags,
            settings: Self::level_settings(console),
            host: self.identity.clone(),
            scripts: crate::level::level_scripts(&self.filesystem, name),
        };
        let seats = self
            .server
            .as_mut()
            .map_or_else(Vec::new, Server::take_seats);
        let collision = Rc::new(CollisionWorld::new(Arc::clone(&map)));
        let (client_end, server_end) = loopback();
        self.server = Some(Server::new(
            Box::new(server_end),
            Rc::clone(&collision),
            start.origin,
            SCRIPTS,
            TICK_SECONDS,
            level.clone(),
        )?);
        if let Some(server) = &mut self.server {
            for seat in seats {
                server.add_seat(seat);
            }
        }
        self.level = level;
        self.collision = Some(Rc::clone(&collision));
        self.client = Some(player_client(Box::new(client_end), collision, start.origin));
        self.step = StepSmoother::default();
        self.look.set_view(f64::from(start.yaw), 0.0);
        self.after_load(&map, palette)
    }

    /// The settings a level starts with.
    fn level_settings(console: &Console<Self>) -> Vec<(String, f64)> {
        let fair = FairSettings::default().cvars().map(|(name, _)| name);
        let rules = GAME_RULES.map(|(name, _)| name);
        fair.iter()
            .chain(&rules)
            .map(|name| {
                let value = console.cvar(name).map_or(0.0, engine_console::Cvar::number);
                ((*name).to_owned(), f64::from(value))
            })
            .collect()
    }

    /// Gets everything ready to show a game on `map` that's just been set up.
    pub(crate) fn after_load(
        &mut self,
        map: &Arc<Map>,
        palette: Palette,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.shot_clock.reset();
        self.gates = StencilGates::default();
        self.flames = Flames::default();
        let map = Arc::clone(map);
        self.map_time = 0.0;
        self.hud = Hud::new();
        self.hud.set_palette(&palette);
        self.kick = ViewKick::default();
        self.paused = false;
        self.menu.close();
        self.script_menu = false;
        self.console_view.set_open(false);
        self.console_view.clear_notify();
        self.audio.stop_sounds();
        let water = self
            .sounds
            .clip(WATER_SOUND, &self.filesystem, &mut self.audio);
        let sky = self
            .sounds
            .clip(SKY_SOUND, &self.filesystem, &mut self.audio);
        self.audio.set_ambient_clips(water, sky);
        self.ambience = Some(AmbientSounds::new(Arc::clone(&map)));
        self.models.clear_numbers();
        self.effects = Effects::default();
        self.zones = ZoneTracker::default();
        self.palette = Some(palette);
        self.map = Some(Arc::clone(&map));
        let authored = self.effects.authored_mut();
        authored.set_effects(&map.effects);
        for point in &map.effect_points {
            authored.start(effect_id(&point.effect), point.origin, None, true);
        }
        if let Some(renderer) = &mut self.renderer {
            self.tile_streamer = TileStreamer::new(&map);
            renderer.load_map(map)?;
            let problems = renderer.take_shader_problems();
            if !problems.is_empty() {
                self.load_warning = Some(problems.join("\n"));
            }
        }
        // An old bake is ignored, which leaves the level bright and flat.
        let name = &self.level.name;
        let unlit = self.map.as_ref().is_some_and(|map| !map.lit);
        if unlit && self.filesystem.read(&format!("maps/{name}.ebake")).is_ok() {
            self.load_warning = Some(format!("{name} has an out of date bake, so it's unlit"));
        }
        Ok(())
    }

    /// A command for the current input. With a menu over a game that can't
    /// stop, the player just stands there.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "f32 keeps angles to a few thousandths of a degree"
    )]
    fn command(&self) -> UserCommand {
        let (yaw, pitch) = (self.look.yaw() as f32, self.look.pitch() as f32);
        if self.is_playing_screen() {
            self.buttons.command(yaw, pitch)
        } else {
            Buttons::default().command(yaw, pitch)
        }
    }

    /// Runs one tick of the game.
    fn run_tick(&mut self, seconds: f32) {
        self.turn_view(seconds);
        let command = self.command();
        self.buttons.impulse = 0;
        let mut bots_due = false;
        if let Some(client) = &mut self.client {
            if let Some(server) = &mut self.server {
                client.send_command(command, seconds);
                server.tick();
                client.receive();
                bots_due = true;
            } else {
                client.receive();
                client.send_command(command, seconds);
            }
            if let Some((yaw, pitch)) = client.take_view() {
                self.look.set_view(f64::from(yaw), f64::from(pitch));
            }
            self.map_time = client.time();
            self.hud.set_stats(client.stats(), client.time());
            let own = client.player_entity();
            self.hud.set_scores(
                client
                    .scores()
                    .iter()
                    .map(|score| Score {
                        name: score.name.clone(),
                        frags: score.frags,
                        colors: score.colors,
                        own: score.entity == own,
                    })
                    .collect(),
            );
            let events = client.take_events();
            self.show_events(events);
        }
        // A held weapon keeps firing with no click to send.
        let holding = self.buttons.attack && self.is_playing_screen();
        if holding {
            self.hold_fire();
        }
        let stopped = !holding || self.weapon_ready().is_none();
        if let (true, Some(client)) = (stopped, &mut self.client) {
            client.let_go_of_fire();
        }
        self.play_sounds();
        if bots_due {
            self.run_bots(seconds);
        }
    }

    /// Text from the server, and a level to change to.
    fn take_server_news(&mut self) -> (Vec<String>, Option<String>) {
        let prints = self.client.as_mut().map(Client::take_prints);
        let level = self.server.as_mut().and_then(Server::take_next_level);
        if level.is_some() {
            self.next_parms = self.server.as_mut().and_then(Server::take_parms);
            self.next_flags = self.server.as_ref().map_or(0.0, Server::flags);
        }
        if let Some(server) = &mut self.server {
            self.commands.extend(server.take_commands());
        }
        let mut prints = prints.unwrap_or_default();
        prints.append(&mut self.ui_news);
        (prints, level)
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
    /// Whether there was a saved config to start from.
    has_config: bool,
    /// Whether this tick's half tick has run.
    half_tick_done: bool,
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
            if !options.white_light {
                mount_coloured_levels(&mut filesystem, &options.base_directory, game);
            }
        }
        let user_directory = saves::user_directory(&options);
        let _created = std::fs::create_dir_all(&user_directory);
        for content in &options.content_directories {
            if content.is_file() {
                filesystem.mount(Box::new(PackArchive::open(content)?));
            } else {
                filesystem.mount(Box::new(DirectoryArchive::new(content)));
            }
        }
        filesystem.mount(Box::new(DirectoryArchive::new(&user_directory)));
        let theme = load_theme(&filesystem);
        let ui_script = load_ui_script(&filesystem, &theme);
        let has_config = user_directory.join(CONFIG_NAME).is_file();
        let keys = Keys::load(&user_directory);
        let mut console = Console::new();
        commands::register(&mut console);
        console.register_cvar("show_input", "0", CvarFlags::default());
        let saved = CvarFlags { saved: true };
        console.register_cvar("m_capture", "1", CvarFlags::default());
        console.register_cvar("password", "", CvarFlags::default());
        console.register_cvar("sv_password", "", CvarFlags::default());
        console.register_cvar("sensitivity", "3", saved);
        console.register_cvar("m_yaw", "0.022", saved);
        console.register_cvar("m_pitch", "0.022", saved);
        console.register_cvar("fov", "90", saved);
        console.register_cvar("show_fps", "1", saved);
        console.register_cvar("vid_vsync", "0", saved);
        console.register_cvar("r_filter", "0", saved);
        console.register_cvar("r_aa", "0", saved);
        console.register_cvar("r_outline", "0", saved);
        console.register_cvar("r_outline_width", "1", saved);
        console.register_cvar("r_outline_fade", "1500", saved);
        console.register_cvar("r_rays", "3", saved);
        console.register_cvar("r_detail_distance", "3000", saved);
        console.register_cvar("r_live_lights", "32", saved);
        console.register_cvar("r_live_shadows", "1", saved);
        console.register_cvar("r_msaa", "1", saved);
        console.register_cvar("r_edges", "0", saved);
        console.register_cvar("r_outline_hull", "0", saved);
        console.register_cvar("r_fog_grid", "0", saved);
        console.register_cvar("r_anisotropy", "8", saved);
        console.register_cvar("volume", "0.7", saved);
        console.register_cvar("bgmvolume", "1", saved);
        console.register_cvar("ambient_level", "0.3", saved);
        console.register_cvar("ambient_water", "0.5", saved);
        console.register_cvar("gamma", "1", saved);
        console.register_cvar("ui_scale", "0", saved);
        console.register_cvar("crosshair", "0", saved);
        console.register_cvar("cl_alwaysrun", "1", saved);
        console.register_cvar("cl_shot_correction", "1", saved);
        console.register_cvar("viewsize", "100", CvarFlags::default());
        console.register_cvar("name", "player", saved);
        console.register_cvar("topcolor", "0", saved);
        console.register_cvar("bottomcolor", "0", saved);
        console.register_cvar("maxplayers", "8", saved);
        console.register_cvar("net_fake_latency", "0", CvarFlags::default());
        console.register_cvar("net_fake_jitter", "0", CvarFlags::default());
        console.register_cvar("net_fake_loss", "0", CvarFlags::default());
        console.register_cvar("net_fake_burst", "0", CvarFlags::default());
        for (name, value) in GAME_RULES {
            console.register_cvar(name, value, CvarFlags::default());
        }
        let server_rule = CvarFlags { saved: true };
        for (name, value) in FairSettings::default().cvars() {
            console.register_cvar(name, &value, server_rule);
        }
        // Not saved, or leaving `e1m8` would carry its gravity into the next game.
        console.register_cvar(
            "sv_gravity",
            &USUAL_GRAVITY.to_string(),
            CvarFlags::default(),
        );
        let rate = TICK_RATE.expect("the tick rate is a non-zero constant");
        Ok(Self {
            console,
            world: World {
                filesystem: Arc::new(filesystem),
                quit: false,
                startup_commands: options.startup_commands,
                ticks: 0,
                frames: 0,
                reduced_frames: 0,
                run_time: Duration::ZERO,
                look: Look::new(LookScale::from_settings(0.0, 0.0, 0.0)),
                motion_events: 0,
                renderer: None,
                tile_streamer: None,
                client: None,
                server: None,
                step: StepSmoother::default(),
                gates: StencilGates::default(),
                flames: Flames::default(),
                live_light_budget: 32,
                buttons: Buttons::default(),
                bindings: Bindings::new(),
                map_time: 0.0,
                load_warning: None,
                fov: DEFAULT_FOV,
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
                effects: Effects::default(),
                effects_time: 0.0,
                zones: ZoneTracker::default(),
                ui_script,
                ui_error: None,
                ui_news: Vec::new(),
                script_menu: false,
                console_settings: BTreeMap::new(),
                theme,
                zone_title: None,
                palette: None,
                next_parms: None,
                next_flags: 0.0,
                level: Level::default(),
                map: None,
                hud: Hud::new(),
                menu: Menu::new(),
                console_view: ConsoleView::new(),
                pictures: PictureBank::default(),
                paused: false,
                kick: ViewKick::default(),
                level_pending: None,
                level_wait: 0,
                commands: Vec::new(),
                ui_scale: 0.0,
                crosshair: false,
                gamma: 1.0,
                rate_text: String::new(),
                menu_settings: MenuSettings::new(),
                user_directory,
                net_server: None,
                connecting: None,
                joining: None,
                keys,
                connect_time: 0.0,
                identity: SeatInfo::default(),
                shot_clock: ShotClock::default(),
                rules_seen: None,
                gravity_shown: USUAL_GRAVITY,
                gravity_seen: USUAL_GRAVITY,
                showing_boxes: false,
                bots: Vec::new(),
                bots_made: 0,
                collision: None,
                room_wait: 0.0,
            },
            clock: TickClock::new(rate, MAX_CATCH_UP),
            last_service: None,
            typed: None,
            size,
            window: None,
            fps: FpsMeter::new(),
            has_config,
            half_tick_done: false,
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
        self.follow_settings();
        // With no game, there's always something to type at.
        if self.world.client.is_none()
            && self.world.connecting.is_none()
            && !self.world.menu.is_open()
            && !self.world.script_menu
            && !self.world.console_view.is_open()
        {
            self.world.console_view.set_open(true);
        }
        self.world.look.set_scale(self.look_scale());
        let filter_mode = self.cvar_number("r_filter");
        let filter = TextureFilter {
            smooth: filter_mode >= 1.0,
            trilinear: filter_mode != 1.0,
            #[expect(clippy::cast_possible_truncation, reason = "a small setting")]
            anisotropy: self.cvar_number("r_anisotropy").clamp(1.0, 16.0) as f32,
        };
        if let Some(renderer) = &mut self.world.renderer
            && let Err(error) = renderer.set_filter(filter)
        {
            self.console.print(format!("filter: {error}"));
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "small settings"
        )]
        let post = PostSettings {
            edge_smoothing: self.cvar_number("r_aa").clamp(0.0, 2.0) as u8,
            outline: self.cvar_number("r_outline").clamp(0.0, 2.0) as u8,
            outline_width: self.cvar_number("r_outline_width") as f32,
            outline_fade: self.cvar_number("r_outline_fade") as f32,
            outline_color: [0.0; 3],
            rays: self.cvar_number("r_rays").clamp(0.0, 3.0) as u8,
            live_shadows: self.cvar_number("r_live_shadows") != 0.0,
            detail_distance: self.cvar_number("r_detail_distance") as f32,
            rounded_edges: self.cvar_number("r_edges").clamp(0.0, 8.0) as f32,
            outline_hull: self.cvar_number("r_outline_hull") != 0.0,
            fog_grid: self.cvar_number("r_fog_grid") != 0.0,
        };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to a small count"
        )]
        let samples = self.cvar_number("r_msaa").clamp(1.0, 8.0) as u32;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "clamped to a small count"
        )]
        let live_budget = self.cvar_number("r_live_lights").clamp(0.0, 64.0) as usize;
        self.world.live_light_budget = live_budget;
        if let Some(renderer) = &mut self.world.renderer {
            renderer.set_post(post);
            if let Err(error) = renderer.set_antialiasing(samples) {
                self.console.print(format!("r_msaa: {error}"));
            }
        }
        let vsync = self.cvar_number("vid_vsync") != 0.0;
        if let Some(renderer) = &mut self.world.renderer {
            renderer.set_vsync(vsync);
        }
        self.world.fov = self
            .console
            .cvar("fov")
            .map_or(DEFAULT_FOV, engine_console::Cvar::number);
        let seconds = TICK_SECONDS;
        self.world.update_network(seconds, &mut self.console);
        if self.world.is_running() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a tick's length needs no more than f32"
            )]
            self.world.run_tick(seconds as f32);
        }
        self.world.follow_level(&mut self.console);
        if let Some(client) = &mut self.world.client {
            client.flush_network();
        }
        if let Some(net_server) = &mut self.world.net_server {
            net_server.flush();
        }
        self.follow_rules();
        let showing = self.console.cvar("sv_unlag_debug");
        self.world.showing_boxes = showing.is_some_and(|cvar| cvar.value() != "0");
        let (prints, level) = self.world.take_server_news();
        for text in prints {
            if text.starts_with(TALK_MARK) {
                self.world.play_local(TALK_SOUND);
            }
            self.console.print(text);
        }
        for command in std::mem::take(&mut self.world.commands) {
            self.console.add(&command);
        }
        if level.is_some() {
            self.world.level_pending = level;
            self.world.level_wait = 0;
        }
        self.change_level();
        self.show_output();
    }

    /// Changes level, once the plaque has had a frame to show.
    fn change_level(&mut self) {
        if self.world.level_wait < PLAQUE_FRAMES {
            return;
        }
        if let Some(level) = self.world.level_pending.take() {
            match self.world.load_map(&level, &self.console) {
                Ok(()) => self.console.print(format!("loaded {level}")),
                Err(error) => self
                    .console
                    .print(format!("couldn't load {level}: {error}")),
            }
        }
    }

    /// Copies what's drawn and heard from the cvars.
    fn follow_settings(&mut self) {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "settings need no more than f32"
        )]
        let number = |engine: &Self, name: &str| engine.cvar_number(name) as f32;
        self.world.ui_scale = number(self, "ui_scale");
        let wanted = self.world.ui_script.as_ref().map(UiScript::wanted_settings);
        for name in wanted.unwrap_or_default() {
            let value = self.cvar_number(&name);
            self.world.console_settings.insert(name, value);
        }
        self.world.crosshair = number(self, "crosshair") != 0.0;
        self.world.gamma = number(self, "gamma").clamp(0.3, 2.0);
        self.world.buttons.always_run = number(self, "cl_alwaysrun") != 0.0;
        let correction = f64::from(number(self, "cl_shot_correction"));
        if let Some(client) = &mut self.world.client {
            client.set_shot_correction(correction);
        }
        let name = self
            .console
            .cvar("name")
            .map_or("player", engine_console::Cvar::value)
            .to_owned();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "colours are from 0 to 13"
        )]
        let colors =
            ((number(self, "topcolor") as u8 & 15) << 4) | (number(self, "bottomcolor") as u8 & 15);
        if name != self.world.identity.name || colors != self.world.identity.colors {
            if let Some(client) = &mut self.world.client {
                client.send_info(&name, colors);
            }
            self.world.identity = SeatInfo { name, colors };
        }
        for name in MENU_SETTINGS {
            let value = number(self, name);
            self.world.menu_settings.insert(name, value);
        }
    }

    /// Keeps the game's rules with the console's, as `StartFrame` and
    /// `cvar_set` do. A level's own gravity shows in `sv_gravity`, and the
    /// console can then change it.
    #[expect(clippy::cast_possible_truncation, reason = "a gravity fits f32")]
    fn follow_rules(&mut self) {
        let rules = LIVE_RULES.map(|name| self.cvar_number(name));
        let wanted = self.cvar_number("sv_gravity") as f32;
        // Only a change counts, or a loaded game would lose its own rules.
        let seen = self.world.rules_seen.replace(rules);
        let (Some(seen), Some(server)) = (seen, &mut self.world.server) else {
            return;
        };
        for ((name, value), old) in LIVE_RULES.iter().zip(rules).zip(seen) {
            if value != old {
                server.set_setting(name, value);
            }
        }
        // Someone changed the cvar, which might have been us, last time.
        if wanted != self.world.gravity_seen {
            self.world.gravity_seen = wanted;
            server.set_gravity(wanted);
            self.world.gravity_shown = wanted;
        }
        let gravity = server.gravity();
        if gravity != self.world.gravity_shown {
            self.world.gravity_shown = gravity;
            self.console.add(&format!("sv_gravity {gravity}"));
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
        if let Some(line) = self.world.load_warning.take() {
            self.console.print(line);
        }
        let lines = self.console.take_output();
        if lines.is_empty() {
            return;
        }
        let mut output = std::io::stdout().lock();
        for line in lines {
            let plain = line.strip_prefix(TALK_MARK).unwrap_or(&line);
            let _ignored = writeln!(output, "{plain}");
            self.world.console_view.print(&line, self.world.map_time);
        }
        let _ignored = output.flush();
    }
}

impl PlatformApp for Engine {
    fn started(&mut self, window: Window) -> Result<(), AppError> {
        let target = Target::Window(window.native()?);
        self.window = Some(window);
        let saved = Some(self.world.user_directory.join("pipelines.cache"));
        let mut renderer = Renderer::keeping_pipelines(target, self.size, saved)?;
        for name in built_in_shaders() {
            if let Ok(bytes) = self.world.filesystem.read(&format!("shaders/{name}.wgsl")) {
                renderer.set_built_in_shader(name, &String::from_utf8_lossy(&bytes));
            }
        }
        self.world.renderer = Some(renderer);
        self.world.look.set_scale(self.look_scale());
        self.typed = Some(read_standard_input());
        self.console.add(&format!("exec {STARTUP_SCRIPT}"));
        if !self.has_config {
            self.console.add(MOVE_BINDINGS);
        }
        Ok(())
    }

    fn service(&mut self, now: Duration) -> ServiceOutcome {
        self.world.run_time = now;
        let elapsed = now.saturating_sub(self.last_service.unwrap_or(now));
        self.last_service = Some(now);
        for _ in 0..self.clock.advance(elapsed) {
            self.tick();
            self.half_tick_done = false;
        }
        self.run_half_tick();
        if self.world.quit {
            ServiceOutcome::Stop
        } else if self.half_tick_done {
            ServiceOutcome::After(self.clock.until_next())
        } else {
            // Wake halfway through, for shots.
            let tick = Duration::from_secs_f64(TICK_SECONDS);
            let until = self.clock.until_next().saturating_sub(tick / 2);
            ServiceOutcome::After(until)
        }
    }

    fn input(&mut self, event: &InputEvent, time: Duration) {
        match *event {
            InputEvent::Motion { x, y } => {
                let held = self
                    .world
                    .client
                    .as_ref()
                    .is_some_and(|client| client.predicted().frozen);
                if !held {
                    self.world.look.motion(x, y);
                }
                self.world.motion_events += 1;
            }
            InputEvent::Key {
                name,
                pressed,
                repeat,
            } => self.key(name, pressed, repeat),
            InputEvent::Button {
                button: MouseButton::Left,
                pressed: true,
            } if self.cvar_number("m_capture") == 0.0 => self.console.add("m_capture 1"),
            InputEvent::Wheel { lines } => {
                let name = if lines > 0.0 {
                    "MWHEELUP"
                } else {
                    "MWHEELDOWN"
                };
                self.key(name, true, false);
                self.key(name, false, false);
            }
            InputEvent::Button { button, pressed } => {
                let name = match button {
                    MouseButton::Left => "MOUSE1",
                    MouseButton::Right => "MOUSE2",
                    MouseButton::Middle => "MOUSE3",
                };
                self.key(name, pressed, false);
            }
            InputEvent::Pointer { .. } => {}
            InputEvent::FocusLost => {
                self.world.buttons.release();
                self.console.add("m_capture 0");
            }
            InputEvent::Resized { width, height } => {
                if let Some(renderer) = &mut self.world.renderer {
                    renderer.resize(width, height);
                }
            }
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
            self.world.rate_text = if self.cvar_number("show_fps") != 0.0 {
                format!("{rate} FPS")
            } else {
                String::new()
            };
        }
        if !frame.reduced {
            let since_service = frame
                .now
                .saturating_sub(self.last_service.unwrap_or(frame.now));
            let tick_seconds = TICK_SECONDS;
            // With the game stopped, what's drawn has to stay put.
            let fraction = if self.world.is_running() {
                self.clock.fraction_after(since_service)
            } else {
                1.0
            };
            let leftover = if self.world.is_running() {
                fraction * tick_seconds
            } else {
                0.0
            };
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a frame's timing needs no more than f32"
            )]
            let (leftover, fraction, seconds) = (
                leftover as f32,
                fraction as f32,
                frame.delta.as_secs_f64() as f32,
            );
            let mut picture = self.world.view(leftover, fraction, seconds);
            let forced = self.world.client.is_none();
            self.world.console_view.update(seconds, forced);
            let size = self
                .world
                .renderer
                .as_ref()
                .map_or(self.size, Renderer::size);
            picture.quads = self.world.overlay(size);
            if let Some(renderer) = &mut self.world.renderer
                && let Err(error) = renderer.draw(&picture.view, picture.drawables())
            {
                self.console.print(format!("renderer: {error}"));
                self.show_output();
                return FrameOutcome::Stop;
            }
        }
        self.world.level_wait = self.world.level_wait.saturating_add(1);
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
            && self.world.is_playing_screen()
            && self.world.client.is_some()
    }

    fn notice(&mut self, message: String) {
        self.console.print(message);
    }

    fn stopping(&mut self) {
        self.show_output();
        saves::write_config(&self.console, &self.world);
        self.world.leave_game();
        self.world.renderer = None;
        self.window = None;
    }
}
