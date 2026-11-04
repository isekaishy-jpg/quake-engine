//! The authoritative game server.
//!
//! It only hears from clients through a transport, even in a local game
//! (proposal 0024). It passes what the simulation says is due to the scripts,
//! one at a time, in slot order (proposal 0025). Each client has a player of
//! their own (proposal 0045).
//!
//! May depend on `engine_core`, `engine_console`, `engine_map`, `engine_physics`, `engine_sim`, `engine_script`, `engine_net`. See proposal 0008.

mod seat;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_core::sound::StaticSound;
use engine_map::Map;
use engine_net::{ClientMessage, PlayerScore, ServerMessage, Snapshot, Transport};
use engine_physics::CollisionWorld;
use engine_script::{Game, SavedScripts, ScriptError};
use engine_sim::{EntityId, SavedSimulation, Simulation};
use serde::{Deserialize, Serialize};

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
}

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
        game.spawn_map(&map.entities);
        let host = simulation.borrow().player_id();
        game.connect_player(host, &level.name, level.parms.as_deref(), "player");
        let mut seat = Seat::new(transport, SeatInfo::default());
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
        let _anything_so_far = game.take_outbox();
        let host = simulation.borrow().player_id();
        let origin = simulation.borrow().player().origin;
        let mut seat = Seat::new(transport, SeatInfo::default());
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
        };
        server.retell(0);
        Ok(server)
    }

    /// Takes in someone who's connected, who's told which map to load and
    /// gets a player when they've loaded it.
    pub fn add_seat(&mut self, mut seat: Seat) {
        seat.player = None;
        seat.shapes_told = 0;
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

    /// Says who the host is, for the scores.
    pub fn set_host_info(&mut self, info: SeatInfo) {
        if let Some(host) = self.seats.first_mut() {
            host.info = info;
        }
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

    /// Hears from everyone.
    fn listen(&mut self) {
        for index in 0..self.seats.len() {
            self.seats[index].add_allowance(self.seconds);
            while let Some(bytes) = self.seats[index].transport.receive() {
                self.seats[index].heard();
                match ClientMessage::decode(&bytes) {
                    Some(ClientMessage::Move(commands, seen)) => {
                        self.seats[index].note_seen(seen);
                        for command in &commands {
                            self.run_command(index, command);
                        }
                    }
                    Some(ClientMessage::Console(text)) => self.console(index, &text),
                    Some(ClientMessage::Ready) => self.seat_ready(index),
                    Some(ClientMessage::Info(info)) => {
                        self.seats[index].info = SeatInfo {
                            name: info.name,
                            colors: info.colors,
                        };
                    }
                    None => {}
                }
            }
        }
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
        self.retell(index);
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
        self.listen();
        self.drop_lost();
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
            Some("say") => {
                let line = text.trim_start().strip_prefix("say").unwrap_or("").trim();
                let name = self.seats[index].info.name.clone();
                self.broadcast(&ServerMessage::Print(format!("{name}: {line}")));
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
