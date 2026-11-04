//! The authoritative game server.
//!
//! It only hears from clients through a transport, even in a local game
//! (proposal 0024). It passes what the simulation says is due to the scripts,
//! one at a time, in slot order (proposal 0025).
//!
//! May depend on `engine_core`, `engine_console`, `engine_map`, `engine_physics`, `engine_sim`, `engine_script`, `engine_net`. See proposal 0008.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_core::sound::StaticSound;
use engine_map::Map;
use engine_net::{ClientMessage, ServerMessage, Snapshot, Transport};
use engine_physics::CollisionWorld;
use engine_script::{Game, SavedScripts, ScriptError};
use engine_sim::{SavedSimulation, Simulation};
use serde::{Deserialize, Serialize};

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
    /// The connection to the client.
    transport: Box<dyn Transport>,
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
    /// What the player carries to it.
    parms: Option<Vec<f64>>,
    /// The game's flags to carry to it.
    flags: f64,
    /// Commands the game gave the host.
    commands: Vec<String>,
    /// The level running, for a restart.
    level: Level,
    /// What the client has been told, for a save.
    told: Told,
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
        game.connect_player(&level.name, level.parms.as_deref());
        let mut server = Self {
            transport,
            world,
            simulation,
            game,
            tick: 0,
            seconds,
            next_level: None,
            parms: None,
            flags: level.flags,
            commands: Vec::new(),
            level,
            told: Told::default(),
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
        let mut server = Self {
            transport,
            world,
            simulation,
            game,
            tick: 0,
            seconds,
            next_level: None,
            parms: None,
            flags: saved.level.flags,
            commands: Vec::new(),
            level: saved.level,
            told: saved.told,
        };
        server.retell();
        Ok(server)
    }

    /// Tells a new client what the old one had heard.
    fn retell(&mut self) {
        for (index, pattern) in self.told.light_styles.clone() {
            self.transport
                .send(ServerMessage::LightStyle { index, pattern }.encode());
        }
        for sound in self.told.static_sounds.clone() {
            self.transport
                .send(ServerMessage::StaticSound(sound).encode());
        }
        if let Some(track) = self.told.music {
            self.transport.send(ServerMessage::Music(track).encode());
        }
        if let Some(title) = self.told.level_title.clone() {
            self.transport
                .send(ServerMessage::LevelTitle(title).encode());
        }
        let (yaw, pitch) = self.simulation.borrow().view();
        self.transport
            .send(ServerMessage::SetView { yaw, pitch }.encode());
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

    /// Where the player is.
    #[must_use]
    pub fn player_origin(&self) -> Vec3 {
        self.simulation.borrow().player().origin
    }

    /// Takes a level the scripts asked for.
    pub fn take_next_level(&mut self) -> Option<String> {
        self.next_level.take()
    }

    /// What the player carries to the level asked for.
    pub fn take_parms(&mut self) -> Option<Vec<f64>> {
        self.parms.take()
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

    /// Runs what the client asked for, moves the world, and tells the client
    /// the result.
    pub fn tick(&mut self) {
        while let Some(bytes) = self.transport.receive() {
            match ClientMessage::decode(&bytes) {
                Some(ClientMessage::Move(command)) => self.run_command(&command),
                Some(ClientMessage::Console(text)) => self.console(&text),
                None => {}
            }
        }
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
        let stats = self.game.client_data();
        self.flush_outbox();
        let shapes = self.simulation.borrow_mut().take_new_shapes();
        for (number, name) in shapes {
            self.transport
                .send(ServerMessage::ModelName { number, name }.encode());
        }
        self.tick = self.tick.wrapping_add(1);
        let simulation = self.simulation.borrow();
        let snapshot = Snapshot {
            tick: self.tick,
            acknowledged: simulation.last_command(),
            player: *simulation.player(),
            instances: simulation.instances(),
            entities: simulation.entity_states(),
            boxes: simulation.solid_boxes(),
            weapon_model: simulation.shown_weapon_model(),
            weapon_frame: simulation.weapon_frame(),
            player_entity: simulation.player_id().to_bits(),
            player_effects: simulation
                .entity(simulation.player_id())
                .map_or(0, |entity| entity.effects),
            punch: simulation.punch(),
            time: simulation.time(),
            stats,
        };
        self.transport
            .send(ServerMessage::Snapshot(snapshot).encode());
        drop(simulation);
        self.simulation.borrow_mut().clear_muzzle_flashes();
    }

    /// Runs a move, with the game's player logic around it.
    fn run_command(&mut self, command: &UserCommand) {
        let fresh = command.sequence > self.simulation.borrow().last_command();
        if fresh {
            self.simulation.borrow_mut().set_buttons(command);
            self.game.player_pre_think();
        }
        self.simulation
            .borrow_mut()
            .run_command(command, &self.world);
        if fresh {
            self.game.player_post_think();
        }
    }

    /// Passes on what the scripts asked for.
    fn flush_outbox(&mut self) {
        let outbox = self.game.take_outbox();
        for (index, pattern) in outbox.light_styles {
            self.told.light_styles.insert(index, pattern.clone());
            self.transport
                .send(ServerMessage::LightStyle { index, pattern }.encode());
        }
        for text in outbox.prints {
            self.transport.send(ServerMessage::Print(text).encode());
        }
        for text in outbox.center_prints {
            self.transport
                .send(ServerMessage::CenterPrint(text).encode());
        }
        for text in outbox.stuffed {
            self.transport.send(ServerMessage::Stuff(text).encode());
        }
        for (armor, blood, from) in outbox.damage {
            self.transport
                .send(ServerMessage::Damage { armor, blood, from }.encode());
        }
        if let Some(kind) = outbox.intermission {
            self.transport
                .send(ServerMessage::Intermission(kind).encode());
        }
        if let Some(text) = outbox.level_title {
            self.told.level_title = Some(text.clone());
            self.transport
                .send(ServerMessage::LevelTitle(text).encode());
        }
        self.commands.extend(outbox.local_commands);
        if let Some((yaw, pitch)) = outbox.view {
            self.transport
                .send(ServerMessage::SetView { yaw, pitch }.encode());
        }
        for sound in outbox.static_sounds {
            self.told.static_sounds.push(sound.clone());
            self.transport
                .send(ServerMessage::StaticSound(sound).encode());
        }
        for sound in outbox.sounds {
            self.transport.send(ServerMessage::Sound(sound).encode());
        }
        if let Some(track) = outbox.music {
            self.told.music = Some(track);
            self.transport.send(ServerMessage::Music(track).encode());
        }
        for effect in outbox.effects {
            self.transport.send(ServerMessage::Effect(effect).encode());
        }
        if outbox.parms.is_some() {
            self.parms = outbox.parms;
        }
        if let Some(flags) = outbox.level_flags {
            self.flags = flags;
        }
        if outbox.restart {
            self.next_level = Some(self.level.name.clone());
            self.parms.clone_from(&self.level.parms);
            self.flags = self.level.flags;
        }
        if outbox.level.is_some() {
            self.next_level = outbox.level;
        }
    }

    /// Runs a console command from the client.
    fn console(&mut self, text: &str) {
        let mut words = text.split_whitespace();
        match words.next() {
            Some("noclip") => {
                let _state = self.simulation.borrow_mut().toggle_noclip();
            }
            Some("setpos") => {
                let mut number = || words.next().and_then(|word| word.parse::<f32>().ok());
                if let (Some(x), Some(y), Some(z)) = (number(), number(), number()) {
                    self.simulation.borrow_mut().teleport(Vec3::new(x, y, z));
                }
            }
            Some(command) => {
                let argument = words.collect::<Vec<_>>().join(" ");
                self.game.client_command(command, &argument);
                self.flush_outbox();
            }
            None => {}
        }
    }
}
