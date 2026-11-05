//! The authoritative game server.
//!
//! It only hears from clients through a transport, even in a local game
//! (proposal 0024). It passes what the simulation says is due to the scripts,
//! one at a time, in slot order (proposal 0025). Each client has a player of
//! their own (proposal 0045).
//!
//! May depend on `engine_core`, `engine_console`, `engine_map`, `engine_physics`, `engine_sim`, `engine_script`, `engine_net`. See proposal 0008.

mod fair;
mod seat;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use engine_core::canvas::TALK_MARK;
use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_core::sound::StaticSound;
use engine_map::Map;
use engine_net::{ClientMessage, PlayerScore, ServerMessage, Snapshot, Transport};
use engine_physics::CollisionWorld;
use engine_script::{Game, SavedScripts, ScriptError};
use engine_sim::{EntityId, SavedSimulation, Simulation};
use serde::{Deserialize, Serialize};

pub use fair::{FairSettings, Zone};
pub use seat::{Seat, SeatInfo};

/// Which level a game starts, and what the player brings to it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Level {
    /// Its name.
    pub name: String,
    /// What the player carries, or `None` for a new game.
    pub parms: Option<Vec<f64>>,
    /// The game's flags carried between levels.
    pub flags: f64,
    /// The host's settings the game can read.
    pub settings: Vec<(String, f64)>,
    /// The host's name and colours.
    #[serde(default)]
    pub host: SeatInfo,
}

/// What's been said to the client that a new one would need to hear again.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
struct Told {
    /// Every light style set, by number.
    light_styles: BTreeMap<u8, String>,
    /// Every sound looping at a fixed place.
    static_sounds: Vec<StaticSound>,
    /// The music playing.
    music: Option<u8>,
    /// The level's title.
    level_title: Option<String>,
    /// The gravity last told of, which saves from before it was kept lack.
    #[serde(default)]
    gravity: Option<f32>,
}

/// A game, written down (proposal 0044).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedGame {
    /// The level, with what it started with.
    level: Level,
    /// The engine's state.
    simulation: SavedSimulation,
    /// The scripts' state.
    scripts: SavedScripts,
    /// What the client has been told.
    told: Told,
}

impl SavedGame {
    /// The level's name.
    #[must_use]
    pub fn level_name(&self) -> &str {
        &self.level.name
    }
}

/// A running game.
pub struct Server {
    /// Everyone connected. The first is the host, who never leaves.
    seats: Vec<Seat>,
    /// The map's collision.
    world: Rc<CollisionWorld>,
    /// The game's state.
    simulation: Rc<RefCell<Simulation>>,
    /// The game's logic.
    game: Game,
    /// Ticks run.
    tick: u32,
    /// The length of a tick.
    seconds: f64,
    /// A level the scripts want to change to.
    next_level: Option<String>,
    /// The game's flags to carry to it.
    flags: f64,
    /// Commands the game gave the host.
    commands: Vec<String>,
    /// The level running, for a restart.
    level: Level,
    /// What the clients have been told.
    told: Told,
    /// Where a new player's entity starts, before the game places them.
    origin: Vec3,
    /// Scores last sent.
    scores: Vec<PlayerScore>,
    /// The fair play rules.
    fair: FairSettings,
    /// The zone now.
    zone: Option<Zone>,
    /// Seconds until the zone is worked out again.
    zone_wait: f64,
}

/// How one player stands, for `status` and tests.
#[derive(Clone, Debug, PartialEq)]
pub struct SeatReport {
    /// Their name.
    pub name: String,
    /// Their ping as measured here, in milliseconds.
    pub ping: f64,
    /// Milliseconds added to their ping.
    pub added: f64,
    /// Milliseconds of it made up for.
    pub compensated: f64,
    /// Whether they're over the ceiling, and get no help.
    pub over_ceiling: bool,
    /// The key they've proved they hold, if they came over the network.
    pub identity: Option<[u8; 32]>,
}

/// How fast added delay and compensation move towards their targets, in
/// milliseconds a second, so timing never jumps in the middle of a fight.
const FAIR_RAMP: f64 = 2.0;

/// The most of one thing said that's passed on.
const MAX_SAY: usize = 128;

/// The longest name, from `client_t`.
const MAX_NAME: usize = 32;

/// `text` with nothing in it that could pass for something else, and no longer
/// than `limit` characters.
fn plain_text(text: &str, limit: usize) -> String {
    text.trim()
        .chars()
        .filter(|character| !character.is_control())
        .take(limit)
        .collect()
}

/// How much later than a shooter's ping a view can be, in seconds: the
/// interpolation buffer, with a tick to spare.
const VIEW_SLACK: f64 = 3.0 / 72.0;

impl Server {
    /// A game of `map` with its player at `origin`, running `sources`, with
    /// ticks of `seconds`.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] if the scripts won't load.
    pub fn new(
        transport: Box<dyn Transport>,
        world: Rc<CollisionWorld>,
        map: &Arc<Map>,
        origin: Vec3,
        sources: &[(&str, &str)],
        seconds: f64,
        level: Level,
    ) -> Result<Self, ScriptError> {
        let simulation = Rc::new(RefCell::new(Simulation::new(Arc::clone(map), origin)));
        simulation.borrow_mut().set_world(Rc::clone(&world));
        let game = Game::new(&simulation, sources)?;
        game.start_level(&level.name, &level.settings, level.flags);
        let fair = FairSettings::from_settings(&level.settings);
        game.spawn_map(&map.entities);
        let host = simulation.borrow().player_id();
        game.connect_player(host, &level.name, level.parms.as_deref(), &level.host.name);
        game.player_info(host, &level.host.name, level.host.colors);
        let mut seat = Seat::new(transport, level.host.clone());
        seat.player = Some(host);
        let mut server = Self {
            seats: vec![seat],
            world,
            simulation,
            game,
            tick: 0,
            seconds,
            next_level: None,
            flags: level.flags,
            commands: Vec::new(),
            level,
            told: Told::default(),
            origin,
            scores: Vec::new(),
            fair,
            zone: None,
            zone_wait: 0.0,
        };
        server.flush_outbox();
        Ok(server)
    }

    /// A game as it was saved, on `map`, running `sources`.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] if the scripts won't load, or don't fit the save.
    pub fn resume(
        transport: Box<dyn Transport>,
        world: Rc<CollisionWorld>,
        map: &Arc<Map>,
        sources: &[(&str, &str)],
        seconds: f64,
        saved: SavedGame,
    ) -> Result<Self, ScriptError> {
        let simulation = Simulation::restored(Arc::clone(map), saved.simulation);
        let simulation = Rc::new(RefCell::new(simulation));
        simulation.borrow_mut().set_world(Rc::clone(&world));
        let game = Game::new(&simulation, sources)?;
        game.start_level(&saved.level.name, &saved.level.settings, saved.level.flags);
        game.restore(&saved.scripts)?;
        let fair = FairSettings::from_settings(&saved.level.settings);
        let _anything_so_far = game.take_outbox();
        let host = simulation.borrow().player_id();
        let origin = simulation.borrow().player().origin;
        let mut seat = Seat::new(transport, saved.level.host.clone());
        seat.player = Some(host);
        let mut server = Self {
            seats: vec![seat],
            world,
            simulation,
            game,
            tick: 0,
            seconds,
            next_level: None,
            flags: saved.level.flags,
            commands: Vec::new(),
            level: saved.level,
            told: saved.told,
            origin,
            scores: Vec::new(),
            fair,
            zone: None,
            zone_wait: 0.0,
        };
        server.retell(0);
        Ok(server)
    }

    /// Takes in someone who's connected, who's told which map to load and
    /// gets a player when they've loaded it.
    pub fn add_seat(&mut self, mut seat: Seat) {
        seat.start_level();
        seat.send(&ServerMessage::Level(self.level.name.clone()));
        self.seats.push(seat);
    }

    /// Takes everyone but the host, to carry them to the next level.
    pub fn take_seats(&mut self) -> Vec<Seat> {
        self.seats.split_off(1)
    }

    /// How many people are in the game.
    #[must_use]
    pub fn seat_count(&self) -> usize {
        self.seats.len()
    }

    /// Tells seat `index` what a client that's just arrived needs to hear.
    fn retell(&mut self, index: usize) {
        let told = self.told.clone();
        let Some(seat) = self.seats.get_mut(index) else {
            return;
        };
        for (index, pattern) in told.light_styles {
            seat.send(&ServerMessage::LightStyle { index, pattern });
        }
        for sound in told.static_sounds {
            seat.send(&ServerMessage::StaticSound(sound));
        }
        if let Some(track) = told.music {
            seat.send(&ServerMessage::Music(track));
        }
        if let Some(title) = told.level_title {
            seat.send(&ServerMessage::LevelTitle(title));
        }
        if let Some(gravity) = told.gravity {
            seat.send(&ServerMessage::Gravity(gravity));
        }
        if let Some(player) = seat.player {
            let (yaw, pitch) = self.simulation.borrow().view(player);
            seat.send(&ServerMessage::SetView { yaw, pitch });
        }
    }

    /// The game as it stands.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] if a script holds something that can't be saved.
    pub fn save(&self) -> Result<SavedGame, ScriptError> {
        Ok(SavedGame {
            level: self.level.clone(),
            simulation: self.simulation.borrow().save(),
            scripts: self.game.save()?,
            told: self.told.clone(),
        })
    }

    /// The level running.
    #[must_use]
    pub fn level(&self) -> &Level {
        &self.level
    }

    /// Where the host's player is.
    #[must_use]
    pub fn player_origin(&self) -> Vec3 {
        self.simulation.borrow().player().origin
    }

    /// Takes a level the scripts asked for.
    pub fn take_next_level(&mut self) -> Option<String> {
        self.next_level.take()
    }

    /// What the host's player carries to the level asked for.
    pub fn take_parms(&mut self) -> Option<Vec<f64>> {
        self.seats.first_mut().and_then(|host| host.parms.take())
    }

    /// The game's flags to carry to the level asked for.
    #[must_use]
    pub fn flags(&self) -> f64 {
        self.flags
    }

    /// Takes the commands the game gave the host.
    pub fn take_commands(&mut self) -> Vec<String> {
        std::mem::take(&mut self.commands)
    }

    /// Runs `source` in the game, for tests and tools.
    ///
    /// # Errors
    ///
    /// Returns [`ScriptError`] if it fails.
    pub fn exec(&self, source: &str) -> Result<(), ScriptError> {
        self.game.exec(source)
    }

    /// Acts on one message from seat `index`.
    fn handle(&mut self, index: usize, message: ClientMessage) {
        match message {
            ClientMessage::Move(commands, seen) => {
                self.seats[index].note_seen(seen, self.seconds);
                self.seats[index].hold_commands(commands);
            }
            ClientMessage::Fire(fire) => self.seats[index].hold_fire(fire),
            ClientMessage::Console(text) => self.console(index, &text),
            ClientMessage::Ready => self.seat_ready(index),
            ClientMessage::Info(info) => {
                let name = plain_text(&info.name, MAX_NAME);
                let name = if name.is_empty() {
                    SeatInfo::default().name
                } else {
                    name
                };
                self.seats[index].info = SeatInfo {
                    name,
                    colors: info.colors,
                };
                self.tell_game_who(index);
            }
        }
    }

    /// Hears from everyone.
    fn listen(&mut self) {
        for index in 0..self.seats.len() {
            self.seats[index].add_allowance(self.seconds);
            while let Some(message) = self.seats[index].backlog.pop_front() {
                self.handle(index, message);
            }
            while let Some(bytes) = self.seats[index].transport.receive() {
                self.seats[index].heard();
                if let Some(message) = ClientMessage::decode(&bytes) {
                    self.handle(index, message);
                }
            }
        }
        self.run_fires(0.0);
        for index in 0..self.seats.len() {
            for command in self.seats[index].take_commands() {
                self.run_command(index, &command);
            }
        }
    }

    /// Runs shots that are due, in the order they were fired, whoever's seat
    /// comes first. `since_tick` is how far past the tick this is, in seconds.
    fn run_fires(&mut self, since_tick: f64) {
        let tick_time = self.simulation.borrow().time();
        let mut fires = Vec::new();
        for index in 0..self.seats.len() {
            let Some(id) = self.seats[index].player else {
                continue;
            };
            for fire in self.seats[index].take_fires(since_tick) {
                // A client can claim the start of a tick, but nothing earlier.
                let into = f64::from(fire.offset) / 256.0 * self.seconds;
                fires.push((tick_time - self.seconds + into, id, fire));
            }
        }
        fires.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (time, id, fire) in fires {
            self.simulation
                .borrow_mut()
                .aim_player(id, fire.yaw, fire.pitch, fire.view_time);
            self.game
                .client_fire(id, time.max(tick_time - self.seconds), fire.seed);
        }
    }

    /// A light pass between ticks, for weapons alone: it reads waiting shots,
    /// tests what they hit and spawns what they launch. Nothing else moves
    /// (proposal 0050).
    pub fn half_tick(&mut self) {
        for index in 0..self.seats.len() {
            while let Some(bytes) = self.seats[index].transport.receive() {
                self.seats[index].heard();
                match ClientMessage::decode(&bytes) {
                    Some(ClientMessage::Fire(fire)) => self.seats[index].hold_fire(fire),
                    Some(message) => self.seats[index].backlog.push_back(message),
                    None => {}
                }
            }
        }
        self.run_fires(self.seconds / 2.0);
        self.flush_outbox();
        for seat in &mut self.seats {
            seat.flush();
        }
    }

    /// Works out the zone from time to time, and moves everyone towards where
    /// it puts them (proposal 0051).
    fn update_fairness(&mut self) {
        self.zone_wait -= self.seconds;
        if self.zone_wait <= 0.0 || self.zone.is_none() {
            self.zone_wait = self.fair.interval;
            let pings: Vec<f64> = self.seats.iter().map(Seat::ping).collect();
            self.zone = Some(Zone::of(&pings, &self.fair));
        }
        let Some(zone) = self.zone else {
            return;
        };
        let step = FAIR_RAMP * self.seconds;
        let rules = self.fair;
        self.simulation.borrow_mut().set_showing_rewind(rules.show);
        for seat in &mut self.seats {
            let ping = seat.ping();
            let (compensated, over) = if rules.enable {
                zone.compensated(ping, seat.over_ceiling, &rules)
            } else {
                (ping, false)
            };
            let added = if rules.enable {
                zone.added_delay(ping)
            } else {
                0.0
            };
            if over != seat.over_ceiling {
                seat.over_ceiling = over;
                let text = if over {
                    "Your ping is over this server's limit: lead your shots"
                } else {
                    "Your ping is back within this server's limit"
                };
                seat.send(&ServerMessage::Print(text.to_owned()));
            }
            if !seat.settled && seat.has_ping() {
                seat.settled = true;
                seat.added = added;
                seat.compensated = compensated;
            }
            seat.added += (added - seat.added).clamp(-step, step);
            // Losing help all at once is the point of the ceiling.
            seat.compensated = if over {
                0.0
            } else {
                seat.compensated + (compensated - seat.compensated).clamp(-step, step)
            };
            if let Some(player) = seat.player {
                let limit = if !rules.unlag || over {
                    0.0
                } else if rules.enable {
                    seat.compensated / 1000.0 + VIEW_SLACK
                } else {
                    1.0
                };
                self.simulation.borrow_mut().set_rewind_limit(player, limit);
            }
        }
    }

    /// How everyone stands.
    #[must_use]
    pub fn report(&self) -> Vec<SeatReport> {
        self.seats
            .iter()
            .map(|seat| SeatReport {
                name: seat.info.name.clone(),
                identity: seat.identity,
                ping: seat.ping(),
                added: seat.added,
                compensated: seat.compensated,
                over_ceiling: seat.over_ceiling,
            })
            .collect()
    }

    /// The zone now, and the rules, for anyone who asks.
    #[must_use]
    pub fn fairness(&self) -> (Option<Zone>, FairSettings) {
        (self.zone, self.fair)
    }

    /// Gives a player to someone who has loaded the map.
    fn seat_ready(&mut self, index: usize) {
        if self.seats[index].player.is_some() {
            return;
        }
        let player = self.simulation.borrow_mut().add_player(self.origin);
        self.game.add_player(player);
        self.seats[index].player = Some(player);
        let saved = self.seats[index].parms.take();
        let name = self.seats[index].info.name.clone();
        self.game
            .connect_player(player, &self.level.name, saved.as_deref(), &name);
        self.tell_game_who(index);
        self.retell(index);
    }

    /// Passes on who the player on `index` is, if they've got a player yet.
    fn tell_game_who(&self, index: usize) {
        let seat = &self.seats[index];
        if let Some(player) = seat.player {
            self.game
                .player_info(player, &seat.info.name, seat.info.colors);
        }
    }

    /// Tells everyone if gravity has changed, since prediction runs on it.
    fn tell_gravity(&mut self) {
        let gravity = self.simulation.borrow().gravity();
        if self.told.gravity != Some(gravity) {
            self.told.gravity = Some(gravity);
            self.broadcast(&ServerMessage::Gravity(gravity));
        }
    }

    /// The level's gravity.
    #[must_use]
    pub fn gravity(&self) -> f32 {
        self.simulation.borrow().gravity()
    }

    /// Sets the level's gravity.
    pub fn set_gravity(&mut self, gravity: f32) {
        self.simulation.borrow_mut().set_gravity(gravity);
    }

    /// Changes a rule in play, as `cvar` sees it.
    pub fn set_setting(&mut self, name: &str, value: f64) {
        self.game.set_setting(name, value);
        match self.level.settings.iter_mut().find(|rule| rule.0 == name) {
            Some(rule) => rule.1 = value,
            None => self.level.settings.push((name.to_owned(), value)),
        }
    }

    /// Puts out the guest called `name`, telling them why. Says whether anyone
    /// was found, and who they've proved to be if they have.
    pub fn kick(&mut self, name: &str, why: &str) -> Option<Option<[u8; 32]>> {
        let seat = self
            .seats
            .iter_mut()
            .skip(1)
            .find(|seat| seat.info.name == name)?;
        seat.send(&ServerMessage::Print(why.to_owned()));
        seat.kick();
        Some(seat.identity)
    }

    /// Lets go of anyone who's left, or gone quiet.
    fn drop_lost(&mut self) {
        let mut index = 1;
        while index < self.seats.len() {
            if self.seats[index].is_lost() {
                let seat = self.seats.remove(index);
                if let Some(player) = seat.player {
                    self.game.disconnect_player(player);
                    self.simulation.borrow_mut().remove_player(player);
                }
            } else {
                index += 1;
            }
        }
    }

    /// Runs what clients asked for, moves the world, and tells clients the
    /// result.
    pub fn tick(&mut self) {
        self.game.start_frame();
        self.listen();
        self.drop_lost();
        self.update_fairness();
        let ids = self.simulation.borrow().ids();
        for id in ids {
            let callbacks = self
                .simulation
                .borrow_mut()
                .advance(id, self.seconds, &self.world);
            for callback in callbacks {
                self.game.run(callback);
            }
        }
        let touches = self.simulation.borrow_mut().take_touches();
        for touch in touches {
            self.game.run(touch);
        }
        self.simulation.borrow_mut().end_tick(self.seconds);
        self.tell_gravity();
        let stats: Vec<Vec<i32>> = self
            .seats
            .iter()
            .map(|seat| {
                seat.player
                    .map_or(Vec::new(), |id| self.game.client_data(id))
            })
            .collect();
        self.flush_outbox();
        self.send_shapes();
        self.send_scores(&stats);
        self.tick = self.tick.wrapping_add(1);
        for (index, stats) in stats.into_iter().enumerate() {
            if let Some(snapshot) = self.snapshot(index, stats) {
                self.seats[index].send_snapshot(snapshot);
            }
        }
        self.simulation.borrow_mut().clear_muzzle_flashes();
        for seat in &mut self.seats {
            seat.flush();
        }
    }

    /// Tells everyone the names of models they haven't heard.
    fn send_shapes(&mut self) {
        let simulation = self.simulation.borrow();
        let shapes = simulation.shapes();
        for seat in &mut self.seats {
            if seat.player.is_none() {
                continue;
            }
            for (index, name) in shapes.iter().enumerate().skip(seat.shapes_told) {
                seat.send(&ServerMessage::ModelName {
                    number: u16::try_from(index + 1).unwrap_or(0),
                    name: name.clone(),
                });
            }
            seat.shapes_told = shapes.len();
        }
    }

    /// Tells everyone the scores, if they've changed. `stats` gives each
    /// player's frags as its second number.
    fn send_scores(&mut self, stats: &[Vec<i32>]) {
        let scores: Vec<PlayerScore> = self
            .seats
            .iter()
            .zip(stats)
            .filter(|(seat, _)| seat.player.is_some())
            .map(|(seat, stats)| PlayerScore {
                entity: seat.player.map_or(0, EntityId::to_bits),
                name: seat.info.name.clone(),
                frags: stats.get(1).copied().unwrap_or(0),
                colors: seat.info.colors,
            })
            .collect();
        if scores != self.scores {
            self.scores.clone_from(&scores);
            self.broadcast(&ServerMessage::Scores(scores));
        }
    }

    /// What seat `index` sees now, if they're in the game.
    fn snapshot(&self, index: usize, stats: Vec<i32>) -> Option<Snapshot> {
        let id = self.seats[index].player?;
        let simulation = self.simulation.borrow();
        Some(Snapshot {
            tick: self.tick,
            acknowledged: simulation.last_command(id),
            player: *simulation.player_of(id)?,
            instances: simulation.instances(),
            entities: simulation.entity_states(),
            boxes: simulation.solid_boxes(id),
            weapon_model: simulation.shown_weapon_model(id),
            weapon_frame: simulation.weapon_frame(id),
            player_entity: id.to_bits(),
            player_effects: simulation.entity(id).map_or(0, |entity| entity.effects),
            punch: simulation.punch(id),
            time: simulation.time(),
            stats,
        })
    }

    /// Runs a move, with the game's player logic around it.
    fn run_command(&mut self, index: usize, command: &UserCommand) {
        let Some(id) = self.seats[index].player else {
            return;
        };
        let fresh = command.sequence > self.simulation.borrow().last_command(id);
        if !fresh || !self.seats[index].spend(command.seconds) {
            return;
        }
        self.simulation.borrow_mut().set_buttons(id, command);
        self.game.player_pre_think(id);
        self.simulation
            .borrow_mut()
            .run_command(id, command, &self.world);
        self.game.player_post_think(id);
    }

    /// `Host_Say`: passes on what the player on `index` said, to their team
    /// alone if it's for the team and teams are being played. The mark tells
    /// clients it's someone talking, and nobody can type it.
    fn say(&mut self, index: usize, line: &str, team_only: bool) {
        let speaker = &self.seats[index].info;
        let (name, team) = (speaker.name.clone(), speaker.colors & 15);
        let line = plain_text(line, MAX_SAY);
        let message = ServerMessage::Print(format!("{TALK_MARK}{name}: {line}"));
        let teams = self
            .level
            .settings
            .iter()
            .any(|(name, value)| name == "teamplay" && *value != 0.0);
        for seat in &mut self.seats {
            let hears = !(teams && team_only) || seat.info.colors & 15 == team;
            if seat.player.is_some() && hears {
                seat.send(&message);
            }
        }
    }

    /// Sends `message` to everyone in the game.
    fn broadcast(&mut self, message: &ServerMessage) {
        for seat in &mut self.seats {
            if seat.player.is_some() {
                seat.send(message);
            }
        }
    }

    /// Sends `message` to the player on `target`, or to everyone.
    fn send_to(&mut self, target: Option<EntityId>, message: &ServerMessage) {
        match target {
            Some(id) => {
                if let Some(seat) = self.seats.iter_mut().find(|seat| seat.player == Some(id)) {
                    seat.send(message);
                }
            }
            None => self.broadcast(message),
        }
    }

    /// Passes on what the scripts asked for.
    fn flush_outbox(&mut self) {
        let outbox = self.game.take_outbox();
        for (index, pattern) in outbox.light_styles {
            self.told.light_styles.insert(index, pattern.clone());
            self.broadcast(&ServerMessage::LightStyle { index, pattern });
        }
        for (target, text) in outbox.prints {
            self.send_to(target, &ServerMessage::Print(text));
        }
        for (target, text) in outbox.center_prints {
            self.send_to(target, &ServerMessage::CenterPrint(text));
        }
        for (target, text) in outbox.stuffed {
            self.send_to(target, &ServerMessage::Stuff(text));
        }
        for (target, armor, blood, from) in outbox.damage {
            self.send_to(Some(target), &ServerMessage::Damage { armor, blood, from });
        }
        if let Some(kind) = outbox.intermission {
            self.broadcast(&ServerMessage::Intermission(kind));
        }
        if let Some(text) = outbox.level_title {
            self.told.level_title = Some(text.clone());
            self.broadcast(&ServerMessage::LevelTitle(text));
        }
        self.commands.extend(outbox.local_commands);
        for (target, yaw, pitch) in outbox.views {
            self.send_to(target, &ServerMessage::SetView { yaw, pitch });
        }
        for sound in outbox.static_sounds {
            self.told.static_sounds.push(sound.clone());
            self.broadcast(&ServerMessage::StaticSound(sound));
        }
        for mut sound in outbox.sounds {
            for seat in &mut self.seats {
                if let Some(player) = seat.player {
                    sound.own = sound.entity == player.to_bits();
                    seat.send_unreliable(&ServerMessage::Sound(sound.clone()));
                }
            }
        }
        if let Some(track) = outbox.music {
            self.told.music = Some(track);
            self.broadcast(&ServerMessage::Music(track));
        }
        for effect in outbox.effects {
            for seat in &mut self.seats {
                if seat.player.is_some() {
                    seat.send_unreliable(&ServerMessage::Effect(effect));
                }
            }
        }
        for (target, parms) in outbox.parms {
            if let Some(seat) = self
                .seats
                .iter_mut()
                .find(|seat| seat.player == Some(target))
            {
                seat.parms = Some(parms);
            }
        }
        if let Some(flags) = outbox.level_flags {
            self.flags = flags;
        }
        if outbox.restart {
            self.next_level = Some(self.level.name.clone());
            let parms = self.level.parms.clone();
            if let Some(host) = self.seats.first_mut() {
                host.parms = parms;
            }
            self.flags = self.level.flags;
        }
        if outbox.level.is_some() {
            self.next_level = outbox.level;
        }
    }

    /// What `status` says, which is how players see the server's rules.
    fn status_lines(&self) -> Vec<String> {
        let rules = self.fair;
        let mut lines = vec![format!(
            "fair play {}, rewind {}, floor {} ms, ceiling {} ms",
            if rules.enable { "on" } else { "off" },
            if rules.unlag { "on" } else { "off" },
            rules.floor,
            rules.ceiling
        )];
        if let Some(zone) = self.zone {
            lines.push(format!(
                "zone {:.0} to {:.0} ms, ceiling {:.0} ms",
                zone.floor, zone.top, zone.ceiling
            ));
        }
        for seat in self.report() {
            let help = if seat.over_ceiling {
                "no help: lead your shots".to_owned()
            } else {
                format!("{:.0} ms made up for", seat.compensated)
            };
            let id = seat.identity.map_or_else(String::new, |key| {
                let start: String = key[..4].iter().map(|byte| format!("{byte:02x}")).collect();
                format!(" [{start}]")
            });
            lines.push(format!(
                "{}{id}: {:.0} ms, {:.0} ms added, {help}",
                seat.name, seat.ping, seat.added
            ));
        }
        lines
    }

    /// Runs a console command from a client. Only the host can cheat.
    fn console(&mut self, index: usize, text: &str) {
        let Some(id) = self.seats[index].player else {
            return;
        };
        let host = index == 0;
        let mut words = text.split_whitespace();
        match words.next() {
            Some("noclip") if host => {
                let _state = self.simulation.borrow_mut().toggle_noclip(id);
            }
            Some("setpos") if host => {
                let mut number = || words.next().and_then(|word| word.parse::<f32>().ok());
                if let (Some(x), Some(y), Some(z)) = (number(), number(), number()) {
                    self.simulation
                        .borrow_mut()
                        .teleport(id, Vec3::new(x, y, z));
                }
            }
            Some("status") => {
                for line in self.status_lines() {
                    self.seats[index].send(&ServerMessage::Print(line));
                }
            }
            Some(command @ ("say" | "say_team")) => {
                let line = text.trim_start().strip_prefix(command).unwrap_or("");
                self.say(index, line, command == "say_team");
            }
            Some(command) if host || matches!(command, "kill") => {
                let argument = words.collect::<Vec<_>>().join(" ");
                self.game.client_command(id, command, &argument);
                self.flush_outbox();
            }
            _ => {}
        }
    }
}
