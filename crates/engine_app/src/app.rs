//! The running engine: the main loop's two halves.

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use engine_assets::DirectoryArchive;
use engine_assets::{AssetError, FileSystem};
use engine_audio::{Audio, Listener};
use engine_client::{
    AmbientSounds, Bindings, Bot, BotSkill, Buttons, Client, DrawnEntity, EffectSource, Effects,
    Look, LookScale, ScreenEvent, ShotClock, StepSmoother, VIEW_HEIGHT, ViewKick, canonical_key,
    dead_roll, view_bob, view_idle, view_roll,
};
use engine_console::{Console, CvarFlags};
use engine_core::canvas::{Quad, TALK_MARK};
use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_core::time::{FpsMeter, TickClock, TickRate};
use engine_map::{Contents, Map};
use engine_net::{Conditioned, Conditions, NetServer, ServerMessage, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_platform::{
    AppError, Frame, FrameOutcome, InputEvent, MouseButton, PlatformApp, ServiceOutcome, Window,
};
use engine_render::{
    Drawables, EntityDraw, LightDraw, ParticleColor, ParticleDraw, Renderer, SpriteDraw, Target,
    View,
};
use engine_server::{Level, Seat, SeatInfo, Server};
use engine_ui::ConsoleView;
use fixture_quake::{
    Hud, Intermission, Menu, Palette, ROCKET_BLAST, SCRIPTS, Score, WeaponRule, import_map,
    mount_game, mount_music, shot_launch, spawn_point, weapon_rule,
};

use crate::commands;
use crate::keys::Keys;
use crate::models::{DrawLists, Lighting, ModelBank};
use crate::options::Options;
use crate::saves;
use crate::screen::{MENU_SETTINGS, MenuActions, MenuSettings, PictureBank};
use crate::sounds::SoundBank;

/// The simulation rate (proposal 0010).
const TICK_RATE: Option<TickRate> = TickRate::new(72);

/// The length of a tick.
pub(crate) const TICK_SECONDS: f64 = 1.0 / 72.0;

/// The most time a stall can catch up on.
const MAX_CATCH_UP: Duration = Duration::from_millis(250);

/// What someone talking sounds like, from `Con_Print`.
const TALK_SOUND: &str = "misc/talk.wav";

/// The name of the saved settings.
pub(crate) const CONFIG_NAME: &str = "config.cfg";

/// How many frames show the loading plaque before a level changes.
const PLAQUE_FRAMES: u8 = 2;

/// Modern movement bindings, set the first time (proposal 0019).
pub(crate) const MOVE_BINDINGS: &str = "bind w +forward; bind s +back; bind a +moveleft; bind d +moveright; \
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

/// How far the eye sits above a dead player's middle.
const DEAD_VIEW_HEIGHT: f32 = -8.0;

/// Settings a game can read, which go to it as each level starts.
const LEVEL_SETTINGS: [&str; 19] = [
    "sv_fair_enable",
    "sv_fair_floor",
    "sv_fair_width",
    "sv_fair_ceiling",
    "sv_fair_ceiling_margin",
    "sv_fair_partial",
    "sv_fair_outlier",
    "sv_fair_interval",
    "sv_unlag_debug",
    "sv_unlag_enable",
    "skill",
    "deathmatch",
    "coop",
    "teamplay",
    "fraglimit",
    "timelimit",
    "samelevel",
    "noexit",
    "registered",
];

/// How long a connection may take to open, in seconds.
const CONNECT_SECONDS: f64 = 10.0;

/// A frame's picture.
pub(crate) struct Picture {
    /// The view.
    pub(crate) view: View,
    /// Where brush models are.
    instances: Vec<ModelInstance>,
    /// The other models.
    entities: Vec<EntityDraw>,
    /// The sprites.
    sprites: Vec<SpriteDraw>,
    /// The particles.
    particles: Vec<ParticleDraw>,
    /// The dynamic lights.
    lights: Vec<LightDraw>,
    /// What's drawn over all of that.
    pub(crate) quads: Vec<Quad>,
}

impl Picture {
    /// What's in it, as the renderer wants it.
    pub(crate) fn drawables(&self) -> Drawables<'_> {
        Drawables {
            instances: &self.instances,
            entities: &self.entities,
            sprites: &self.sprites,
            particles: &self.particles,
            lights: &self.lights,
            quads: &self.quads,
        }
    }
}

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
    /// What the client shows of what's happening.
    pub(crate) effects: Effects,
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
    /// Whether shots show the boxes on this screen, from `sv_unlag_debug`.
    showing_boxes: bool,
    /// Bots playing here, each a client of its own.
    pub(crate) bots: Vec<Bot>,
    /// How many bots have been made, for names and seeds.
    bots_made: u32,
    /// The map's collision, for bots.
    collision: Option<Rc<CollisionWorld>>,
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
        !self.menu.is_open() && !self.console_view.is_open()
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

    /// Adds a bot on a connection of `ping` milliseconds. Returns its name.
    pub(crate) fn add_bot(&mut self, ping: u64) -> Option<String> {
        let world = Rc::clone(self.collision.as_ref()?);
        let server = self.server.as_mut()?;
        self.bots_made += 1;
        let number = self.bots_made;
        let conditions = Conditions {
            send_delay: Duration::from_millis(ping / 2),
            ..Conditions::default()
        };
        let (bot_end, server_end) = loopback();
        let seed = u64::from(number) * 7919;
        let name = format!("bot{number}");
        let colors = u8::try_from(number % 14).unwrap_or(0);
        let server_end = Conditioned::new(Box::new(server_end), conditions, seed);
        server.add_seat(Seat::new(
            Box::new(server_end),
            SeatInfo {
                name: name.clone(),
                colors: colors << 4 | (13 - colors),
            },
        ));
        let bot_end = Conditioned::new(Box::new(bot_end), conditions, seed + 1);
        let mut client = Client::new(Box::new(bot_end), world, Vec3::ZERO);
        client.send_ready();
        self.bots
            .push(Bot::new(client, BotSkill::default(), number));
        Some(name)
    }

    /// Plays a tick of every bot, following the server to a new level.
    fn run_bots(&mut self, seconds: f32) {
        let Some(world) = self.collision.clone() else {
            return;
        };
        for bot in std::mem::take(&mut self.bots) {
            let mut bot = bot;
            bot.tick(seconds, &world);
            if bot.client_mut().take_level().is_some() {
                let (transport, skill, seed) = bot.into_parts();
                let mut client = Client::new(transport, Rc::clone(&world), Vec3::ZERO);
                client.send_ready();
                bot = Bot::new(client, skill, seed);
            }
            self.bots.push(bot);
        }
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
        let bsp = self.filesystem.read(&format!("maps/{name}.bsp"))?;
        let lit = self.filesystem.read(&format!("maps/{name}.lit")).ok();
        let palette = self.filesystem.read("gfx/palette.lmp")?;
        let palette = Palette::parse(&palette).ok_or("gfx/palette.lmp is the wrong size")?;
        let map = Arc::new(import_map(&bsp, lit.as_deref(), &palette)?);
        let start = spawn_point(&map);
        let level = Level {
            name: name.to_owned(),
            parms: self.next_parms.take(),
            flags: self.next_flags,
            settings: Self::level_settings(console),
            host: self.identity.clone(),
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
            &map,
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

    /// Starts connecting to a server, leaving any game going.
    pub(crate) fn begin_connection(&mut self, transport: Box<dyn Transport>) {
        self.leave_game();
        self.connecting = Some(transport);
        self.connect_time = 0.0;
    }

    /// Leaves whatever game is going.
    pub(crate) fn leave_game(&mut self) {
        if let Some(net_server) = &mut self.net_server {
            net_server.close();
            net_server.flush();
        }
        self.net_server = None;
        self.bots.clear();
        self.console_view.cancel_chat();
        self.collision = None;
        self.connecting = None;
        self.client = None;
        self.server = None;
        self.map = None;
        self.level = Level::default();
        self.level_pending = None;
        self.audio.stop_sounds();
        self.effects = Effects::default();
    }

    /// Loads the map a server named, and tells it so.
    fn join_level(
        &mut self,
        name: &str,
        transport: Box<dyn Transport>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let bsp = self.filesystem.read(&format!("maps/{name}.bsp"))?;
        let lit = self.filesystem.read(&format!("maps/{name}.lit")).ok();
        let palette = self.filesystem.read("gfx/palette.lmp")?;
        let palette = Palette::parse(&palette).ok_or("gfx/palette.lmp is the wrong size")?;
        let map = Arc::new(import_map(&bsp, lit.as_deref(), &palette)?);
        let start = spawn_point(&map);
        let collision = Rc::new(CollisionWorld::new(Arc::clone(&map)));
        let mut client = player_client(transport, collision, start.origin);
        client.send_info(&self.identity.name, self.identity.colors);
        client.send_ready();
        client.flush_network();
        self.client = Some(client);
        self.step = StepSmoother::default();
        self.after_load(&map, palette)
    }

    /// Hears from the network, and lets anyone who's arrived in.
    fn update_network(&mut self, seconds: f64, console: &mut Console<Self>) {
        let elapsed = Duration::from_secs_f64(seconds);
        if let Some(net_server) = &mut self.net_server {
            net_server.update(elapsed);
            while let Some(transport) = net_server.take_arrival() {
                if let Some(server) = &mut self.server {
                    server.add_seat(Seat::new(transport, SeatInfo::default()));
                }
            }
            if net_server.take_identity_changed() {
                self.keys.host = net_server.identity();
                self.keys.save_host();
            }
        }
        if let Some(client) = &mut self.client {
            client.update_network(elapsed);
        }
        self.update_connecting(seconds, elapsed, console);
        let lost = self.server.is_none()
            && self
                .client
                .as_ref()
                .is_some_and(|client| !client.is_connected());
        if lost {
            console.print("the server has gone");
            self.leave_game();
        }
    }

    /// Waits for a connection to name its level.
    fn update_connecting(&mut self, seconds: f64, elapsed: Duration, console: &mut Console<Self>) {
        let Some(mut transport) = self.connecting.take() else {
            return;
        };
        transport.update(elapsed);
        transport.flush();
        self.connect_time += seconds;
        let level =
            std::iter::from_fn(|| transport.receive()).find_map(
                |bytes| match ServerMessage::decode(&bytes) {
                    Some(ServerMessage::Level(name)) => Some(name),
                    _ => None,
                },
            );
        if let Some(level) = level {
            if let (Some(address), Some(key)) = (self.joining.take(), transport.peer_identity()) {
                self.keys.remember_host(address, key);
            }
            if let Err(error) = self.join_level(&level, transport) {
                console.print(format!("couldn't load {level}: {error}"));
                self.leave_game();
            }
        } else if !transport.is_connected() || self.connect_time > CONNECT_SECONDS {
            let reason = transport.refusal();
            let reason = reason.as_deref().unwrap_or("no answer");
            console.print(format!("couldn't connect: {reason}"));
        } else {
            self.connecting = Some(transport);
        }
    }

    /// Follows the server to a new level.
    fn follow_level(&mut self, console: &mut Console<Self>) {
        let wanted = self.client.as_mut().and_then(Client::take_level);
        if let (Some(name), None) = (wanted, &self.server)
            && let Some(client) = self.client.take()
            && let Err(error) = self.join_level(&name, client.into_transport())
        {
            console.print(format!("couldn't load {name}: {error}"));
            self.leave_game();
        }
    }

    /// The settings a level starts with.
    fn level_settings(console: &Console<Self>) -> Vec<(String, f64)> {
        LEVEL_SETTINGS
            .iter()
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
        let map = Arc::clone(map);
        self.map_time = 0.0;
        self.hud = Hud::new();
        self.hud.set_palette(&palette);
        self.kick = ViewKick::default();
        self.paused = false;
        self.menu.close();
        self.console_view.set_open(false);
        self.console_view.clear_notify();
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
        self.effects = Effects::default();
        self.palette = Some(palette);
        self.map = Some(Arc::clone(&map));
        if let Some(renderer) = &mut self.renderer {
            renderer.load_map(map)?;
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

    /// The rule of a shot that can go now, moving the time of the next one on.
    /// The second part says whether this carries on from the last shot.
    fn next_shot(&mut self) -> Option<(WeaponRule, bool)> {
        let ammo = self.hud.ammo();
        let rule = weapon_rule(self.hud.active_weapon(), ammo)?;
        // A click from the dead is a wish to come back, not a shot.
        let idle = self.hud.is_dead() || self.hud.intermission() != Intermission::None;
        if self.server.is_some() || ammo < rule.ammo || idle {
            return None;
        }
        let carrying_on = self.shot_clock.take(self.map_time, rule.refire)?;
        Some((rule, carrying_on))
    }

    /// Shows a shot of the player's before the server answers. Nothing here
    /// decides a result (proposals 0052 and 0057).
    #[expect(
        clippy::cast_possible_truncation,
        reason = "f32 keeps angles to a few thousandths of a degree"
    )]
    fn guess_shot(&mut self, rule: &WeaponRule, carrying_on: bool, seed: u16) {
        let (yaw, pitch) = (self.look.yaw() as f32, self.look.pitch() as f32);
        let (yaw_sin, yaw_cos) = yaw.to_radians().sin_cos();
        let (pitch_sin, pitch_cos) = pitch.to_radians().sin_cos();
        let axes = [
            Vec3::new(yaw_cos * pitch_cos, yaw_sin * pitch_cos, -pitch_sin),
            Vec3::new(yaw_sin, -yaw_cos, 0.0),
            Vec3::new(yaw_cos * pitch_sin, yaw_sin * pitch_sin, pitch_cos),
        ];
        let sounds = !(rule.sound_once && carrying_on);
        if let Some(client) = &mut self.client {
            if let Some(shot) = &rule.shot {
                let origin = client.predicted().origin;
                let (start, velocity) = shot_launch(shot, origin, axes, pitch, seed);
                let kind = (shot.bounces, shot.blasts, shot.fuse);
                client.guess_launch(shot.model, start, velocity, kind);
            }
            client.show_shot(rule.feel);
            if sounds {
                client.expect_own_sound(rule.sound);
            }
        }
        if sounds {
            self.play_local(rule.sound);
        }
    }

    /// Keeps a held weapon going. What goes round on its own needs no more
    /// shots sent, and the rest go out as a click's would, with their seeds.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "f32 keeps angles to a few thousandths of a degree"
    )]
    fn hold_fire(&mut self) {
        let Some((rule, carrying_on)) = self.next_shot() else {
            return;
        };
        let (yaw, pitch) = (self.look.yaw() as f32, self.look.pitch() as f32);
        let seed = match &mut self.client {
            Some(client) if !rule.feel.repeats => client.send_fire(yaw, pitch, 0),
            _ => 0,
        };
        self.guess_shot(&rule, carrying_on, seed);
        if self.showing_boxes {
            self.show_seen_boxes();
        }
    }

    /// Shows the other players' boxes as they are on this screen.
    fn show_seen_boxes(&mut self) {
        let Some(client) = &mut self.client else {
            return;
        };
        let players: Vec<u64> = client.scores().iter().map(|score| score.entity).collect();
        let drawn = client.draw_entities_buffered(0.0);
        for entity in drawn {
            if players.contains(&entity.state.id) {
                self.effects.show_seen_box(entity.state.origin);
            }
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
        if self.buttons.attack && self.is_playing_screen() {
            self.hold_fire();
        } else if let Some(client) = &mut self.client {
            client.let_go_of_fire();
        }
        self.play_sounds();
        if bots_due {
            self.run_bots(seconds);
        }
    }

    /// Turns the view by the keys held.
    fn turn_view(&mut self, seconds: f32) {
        let (yaw, pitch) = self.buttons.turn(seconds);
        let held = self
            .client
            .as_ref()
            .is_some_and(|client| client.predicted().frozen);
        if !held && (yaw != 0.0 || pitch != 0.0) {
            let (yaw, pitch) = (
                self.look.yaw() + f64::from(yaw),
                self.look.pitch() + f64::from(pitch),
            );
            self.look.set_view(yaw, pitch);
        }
    }

    /// Shows what the game asked to.
    fn show_events(&mut self, events: Vec<ScreenEvent>) {
        for event in events {
            match event {
                ScreenEvent::CenterPrint(text) => self.hud.center_print(text),
                ScreenEvent::Command(text) => self.commands.push(text),
                ScreenEvent::Damage { armor, blood, from } => self.hurt(armor, blood, from),
                ScreenEvent::Intermission(kind) => {
                    self.hud.set_intermission(Intermission::from_number(kind));
                }
                ScreenEvent::LevelTitle(text) => {
                    self.console_view.print(&text, f64::NEG_INFINITY);
                    self.hud.set_level_title(text);
                }
            }
        }
    }

    /// Shows the player being hurt, from `V_ParseDamage`.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "f32 keeps angles to a few thousandths of a degree"
    )]
    fn hurt(&mut self, armor: u8, blood: u8, from: Vec3) {
        self.hud.damage(armor, blood);
        let Some(client) = &self.client else {
            return;
        };
        let count = (f32::from(blood) * 0.5 + f32::from(armor) * 0.5).max(10.0);
        let direction = (from - client.predicted().origin).normalized();
        let (yaw_sin, yaw_cos) = (self.look.yaw() as f32).to_radians().sin_cos();
        let (pitch_sin, pitch_cos) = (self.look.pitch() as f32).to_radians().sin_cos();
        let forward = Vec3::new(yaw_cos * pitch_cos, yaw_sin * pitch_cos, -pitch_sin);
        let right = Vec3::new(yaw_sin, -yaw_cos, 0.0);
        self.kick
            .hit(count, direction.dot(right), direction.dot(forward));
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
        for effect in client.take_effects() {
            self.effects.start(&effect);
        }
        let mut sounds = client.take_sounds();
        sounds.extend(self.effects.take_sounds());
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
        if level.is_some() {
            self.next_parms = self.server.as_mut().and_then(Server::take_parms);
            self.next_flags = self.server.as_ref().map_or(0.0, Server::flags);
        }
        if let Some(server) = &mut self.server {
            self.commands.extend(server.take_commands());
        }
        (prints.unwrap_or_default(), level)
    }

    /// What's at the eye, for the colour of the view.
    fn contents_at(&self, eye: Vec3) -> Contents {
        self.map.as_ref().map_or(Contents::Empty, |map| {
            map.tree
                .leaf(map.tree.leaf_at(eye))
                .map_or(Contents::Empty, |leaf| leaf.contents)
        })
    }

    /// Follows what's on the client for effects, and returns the lights
    /// shining now.
    fn update_effects(
        &mut self,
        time: f32,
        drawn: &[DrawnEntity],
        player: EffectSource,
    ) -> Vec<LightDraw> {
        self.effects.update(time);
        let mut sources: Vec<EffectSource> = drawn
            .iter()
            .map(|entity| EffectSource {
                id: entity.state.id,
                origin: entity.state.origin,
                angles: entity.state.angles,
                effects: entity.state.effects,
                model_flags: self.models.flags(entity.state.model),
            })
            .collect();
        sources.push(player);
        self.effects.follow(&sources);
        self.effects
            .lights()
            .map(|light| LightDraw {
                origin: light.origin,
                radius: light.radius,
                min_light: light.min_light,
            })
            .collect()
    }

    /// The particles to draw, in their colours.
    fn particle_draws(&self) -> Vec<ParticleDraw> {
        let Some(palette) = &self.palette else {
            return Vec::new();
        };
        self.effects
            .particles()
            .live()
            .iter()
            .map(|particle| {
                let [red, green, blue] = palette.color(particle.color);
                ParticleDraw {
                    origin: particle.origin,
                    color: ParticleColor { red, green, blue },
                }
            })
            .collect()
    }

    /// The picture, `leftover` seconds past the last tick. `frame_seconds` is
    /// the time since the last view, for smoothing.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "angles and map time need no more than f32 for drawing"
    )]
    pub(crate) fn view(&mut self, leftover: f32, fraction: f32, frame_seconds: f32) -> Picture {
        let command = self.command();
        let mut origin = Vec3::ZERO;
        let mut instances = Vec::new();
        let mut lists = DrawLists::default();
        let mut lights = Vec::new();
        let mut styles = [1.0; engine_client::LIGHT_STYLE_COUNT];
        let mut punch = Vec3::ZERO;
        let online = self.is_online();
        let mut sway = (0.0, 0.0, 0.0);
        let mut roll = 0.0;
        let time = self.map_time as f32;
        if let Some(client) = &mut self.client {
            let state = client.render_state(&command, leftover);
            let body = state.origin + client.ground_lag(&state, fraction);
            let step = self.step.update(body.z, state.on_ground, frame_seconds);
            let speed = Vec3::new(state.velocity.x, state.velocity.y, 0.0).length();
            let bob = view_bob(time, speed);
            let frozen = state.frozen;
            let eye = if frozen {
                0.0
            } else if state.dead {
                DEAD_VIEW_HEIGHT
            } else {
                VIEW_HEIGHT
            };
            origin = body + Vec3::new(0.0, 0.0, eye + step + bob);
            let (sin, cos) = self.look.yaw().to_radians().sin_cos();
            self.listener = Listener {
                origin,
                right: Vec3::new(sin as f32, -cos as f32, 0.0),
            };
            instances = client.draw_instances(fraction);
            styles = client.light_styles(self.map_time);
            punch = client.punch(fraction);
            let (kick_pitch, kick_roll) = self.kick.update(frame_seconds);
            let right = self.listener.right;
            roll = view_roll(state.velocity.dot(right)) + kick_roll;
            punch.x += kick_pitch;
            if state.dead {
                roll = dead_roll();
            }
            if frozen {
                roll = 0.0;
                sway = view_idle(time);
            }
            let drawn = if online {
                client.draw_entities_buffered(f64::from(frame_seconds))
            } else {
                client.draw_entities(fraction)
            };
            let wearing: HashMap<u64, u8> = client
                .scores()
                .iter()
                .map(|score| (score.entity, score.colors))
                .collect();
            let (model, frame) = client.weapon();
            let player = EffectSource {
                id: client.player_entity(),
                origin: body,
                angles: Vec3::new(self.look.pitch() as f32, self.look.yaw() as f32, 0.0),
                effects: client.player_effects(),
                model_flags: 0,
            };
            lights = self.update_effects(time + leftover, &drawn, player);
            if let Some(map) = self.map.clone() {
                let lighting = Lighting {
                    map: &map,
                    styles: &styles,
                    lights: &lights,
                    time,
                };
                for entity in &drawn {
                    let colors = wearing.get(&entity.state.id).copied();
                    if let Some(colors) = colors {
                        self.models.prepare_colors(
                            entity.state.model,
                            colors,
                            &self.filesystem,
                            self.renderer.as_mut(),
                        );
                    }
                    self.models.draw(entity, colors, &lighting, &mut lists);
                }
                for piece in self.effects.beam_pieces(player.id, body) {
                    self.models.draw_beam(
                        &piece,
                        &self.filesystem,
                        self.renderer.as_mut(),
                        &mut lists,
                    );
                }
                let yaw = self.look.yaw() as f32;
                let pitch = self.look.pitch() as f32;
                let (yaw_sin, yaw_cos) = yaw.to_radians().sin_cos();
                let (pitch_sin, pitch_cos) = pitch.to_radians().sin_cos();
                let forward = Vec3::new(yaw_cos * pitch_cos, yaw_sin * pitch_cos, -pitch_sin);
                // From `V_CalcRefdef`, with the status bar's 2 units.
                let placed = origin + forward * (bob * 0.4) + Vec3::new(0.0, 0.0, 2.0);
                let angles = Vec3::new(-pitch, yaw, 0.0);
                if !frozen {
                    lists.entities.extend(self.models.draw_weapon(
                        u32::from(model),
                        frame,
                        (placed, angles),
                        &lighting,
                    ));
                }
            }
        }
        let contents = self.contents_at(origin);
        let fade = if self.is_running() {
            frame_seconds
        } else {
            0.0
        };
        self.hud.update_view(fade, contents);
        let view = View {
            origin,
            yaw: self.look.yaw() as f32 + punch.y + sway.1,
            pitch: self.look.pitch() as f32 + punch.x + sway.0,
            roll: roll + sway.2,
            fov: self.fov,
            time: self.map_time as f32,
            styles,
            gamma: self.gamma,
        };
        Picture {
            view,
            instances,
            entities: lists.entities,
            sprites: lists.sprites,
            particles: self.particle_draws(),
            lights,
            quads: Vec::new(),
        }
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
        }
        let user_directory = saves::user_directory(&options);
        let _created = std::fs::create_dir_all(&user_directory);
        filesystem.mount(Box::new(DirectoryArchive::new(&user_directory)));
        let has_config = user_directory.join(CONFIG_NAME).is_file();
        let keys = Keys::load(&user_directory);
        let mut console = Console::new();
        commands::register(&mut console);
        console.register_cvar("show_input", "0", CvarFlags::default());
        let saved = CvarFlags {
            saved: true,
            server: false,
        };
        console.register_cvar("m_capture", "1", CvarFlags::default());
        console.register_cvar("password", "", CvarFlags::default());
        console.register_cvar("sv_password", "", CvarFlags::default());
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
        console.register_cvar("gamma", "1", saved);
        console.register_cvar("ui_scale", "0", saved);
        console.register_cvar("crosshair", "0", saved);
        console.register_cvar("cl_alwaysrun", "1", saved);
        console.register_cvar("viewsize", "100", CvarFlags::default());
        console.register_cvar("name", "player", saved);
        console.register_cvar("topcolor", "0", saved);
        console.register_cvar("bottomcolor", "0", saved);
        console.register_cvar("maxplayers", "8", saved);
        console.register_cvar("net_fake_latency", "0", CvarFlags::default());
        console.register_cvar("net_fake_jitter", "0", CvarFlags::default());
        console.register_cvar("net_fake_loss", "0", CvarFlags::default());
        for (name, value) in [
            ("skill", "1"),
            ("deathmatch", "0"),
            ("coop", "0"),
            ("teamplay", "0"),
            ("fraglimit", "0"),
            ("timelimit", "0"),
            ("samelevel", "0"),
            ("noexit", "0"),
            ("registered", "1"),
        ] {
            console.register_cvar(name, value, CvarFlags::default());
        }
        let server_rule = CvarFlags {
            saved: true,
            server: true,
        };
        for (name, value) in [
            ("sv_fair_enable", "1"),
            ("sv_fair_floor", "20"),
            ("sv_fair_width", "20"),
            ("sv_fair_ceiling", "90"),
            ("sv_fair_ceiling_margin", "5"),
            ("sv_fair_partial", "0.5"),
            ("sv_fair_outlier", "100"),
            ("sv_fair_interval", "30"),
            ("sv_unlag_debug", "0"),
            ("sv_unlag_enable", "1"),
        ] {
            console.register_cvar(name, value, server_rule);
        }
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
                effects: Effects::default(),
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
                showing_boxes: false,
                bots: Vec::new(),
                bots_made: 0,
                collision: None,
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
            && !self.world.console_view.is_open()
        {
            self.world.console_view.set_open(true);
        }
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
        self.world.crosshair = number(self, "crosshair") != 0.0;
        self.world.gamma = number(self, "gamma").clamp(0.3, 2.0);
        self.world.buttons.always_run = number(self, "cl_alwaysrun") != 0.0;
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

    /// Takes a key, which goes to what's on top, as `Key_Event`.
    fn key(&mut self, name: &str, pressed: bool, repeat: bool) {
        let key = canonical_key(name);
        if key == "SHIFT" {
            self.world.console_view.set_shift(pressed);
        }
        if !pressed {
            // Key ups always go to the game, so nothing stays held.
            if let Some(command) = self.world.bindings.release(&key) {
                self.console.add(&command);
            }
            return;
        }
        if self.world.menu.is_open() {
            let mut menu = std::mem::take(&mut self.world.menu);
            menu.key(
                &key,
                &mut MenuActions {
                    console: &mut self.console,
                    world: &mut self.world,
                },
            );
            self.world.menu = menu;
        } else if self.world.console_view.is_open() {
            self.console_key(&key);
        } else if self.world.console_view.is_chatting() {
            if let Some(line) = self.world.console_view.chat_key(&key) {
                self.console.add(&line);
            }
        } else if key == "ESCAPE" {
            if !repeat {
                self.console.add("togglemenu");
            }
        } else if !repeat && let Some(command) = self.world.bindings.press(&key) {
            let command = command.to_owned();
            if command == "+attack" {
                self.fire_now();
            }
            self.console.add(&command);
        }
    }

    /// Sends a shot now, without waiting for the console or the next tick.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "f32 keeps angles well, and a share of 256 fits a byte"
    )]
    fn fire_now(&mut self) {
        if !self.world.is_running() || !self.world.is_playing_screen() {
            return;
        }
        let offset = (self.clock.fraction() * 256.0) as u8;
        let (yaw, pitch) = (self.world.look.yaw() as f32, self.world.look.pitch() as f32);
        if let Some(client) = &mut self.world.client {
            let seed = client.send_fire(yaw, pitch, offset);
            if let Some((rule, carrying_on)) = self.world.next_shot() {
                self.world.guess_shot(&rule, carrying_on, seed);
            }
        }
        if self.world.showing_boxes {
            self.world.show_seen_boxes();
        }
    }

    /// Runs the half tick, once in each tick, when it's due.
    fn run_half_tick(&mut self) {
        if !self.half_tick_done && self.clock.fraction() >= 0.5 {
            self.half_tick_done = true;
            if self.world.is_running()
                && let Some(server) = &mut self.world.server
            {
                server.half_tick();
            }
        }
    }

    /// Takes a key while the console is open.
    fn console_key(&mut self, key: &str) {
        match key {
            "ESCAPE" => self.console.add("togglemenu"),
            "`" | "~" => self.console.add("toggleconsole"),
            "TAB" => self.complete(),
            "MWHEELUP" => {
                let _nothing = self.world.console_view.key("PGUP");
            }
            "MWHEELDOWN" => {
                let _nothing = self.world.console_view.key("PGDN");
            }
            _ => {
                if let Some(line) = self.world.console_view.key(key) {
                    self.console.add(&line);
                }
            }
        }
    }

    /// Completes what's being typed, as far as every match agrees.
    fn complete(&mut self) {
        let typed = self.world.console_view.input().trim_start().to_owned();
        if typed.is_empty() || typed.contains(' ') {
            return;
        }
        let matches = self.console.completions(&typed);
        let Some(first) = matches.first() else {
            return;
        };
        let shared = matches.iter().fold(first.len(), |shared, name| {
            first
                .bytes()
                .zip(name.bytes())
                .take(shared)
                .take_while(|(a, b)| a == b)
                .count()
        });
        let mut completed = first[..shared].to_owned();
        if let [_one] = matches.as_slice() {
            completed.push(' ');
        } else {
            for name in &matches {
                self.world
                    .console_view
                    .print(&format!("  {name}"), f64::NEG_INFINITY);
            }
        }
        self.world.console_view.set_input(&completed);
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
        self.world.renderer = Some(Renderer::new(target, self.size)?);
        self.world.look.set_scale(self.look_scale());
        self.typed = Some(read_standard_input());
        self.console.add("exec quake.rc");
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
            let tick_seconds = 1.0 / f64::from(self.clock.rate().hertz());
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
