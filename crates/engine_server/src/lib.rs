//! The authoritative game server.
//!
//! It only hears from clients through a transport, even in a local game
//! (proposal 0024). It passes what the simulation says is due to the scripts,
//! one at a time, in slot order (proposal 0025).
//!
//! May depend on `engine_core`, `engine_console`, `engine_map`, `engine_physics`, `engine_sim`, `engine_script`, `engine_net`. See proposal 0008.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_map::Map;
use engine_net::{ClientMessage, ServerMessage, Snapshot, Transport};
use engine_physics::CollisionWorld;
use engine_script::{Game, ScriptError};
use engine_sim::Simulation;

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
    ) -> Result<Self, ScriptError> {
        let simulation = Rc::new(RefCell::new(Simulation::new(Arc::clone(map), origin)));
        let game = Game::new(&simulation, sources)?;
        game.spawn_map(&map.entities);
        let mut server = Self {
            transport,
            world,
            simulation,
            game,
            tick: 0,
            seconds,
            next_level: None,
        };
        server.flush_outbox();
        Ok(server)
    }

    /// Takes a level the scripts asked for.
    pub fn take_next_level(&mut self) -> Option<String> {
        self.next_level.take()
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
            weapon_model: simulation.weapon_model(),
            weapon_frame: simulation.weapon_frame(),
        };
        self.transport
            .send(ServerMessage::Snapshot(snapshot).encode());
    }

    /// Runs a move, with the game's player logic around it.
    fn run_command(&mut self, command: &UserCommand) {
        let fresh = command.sequence > self.simulation.borrow().last_command();
        if fresh {
            self.simulation.borrow_mut().set_jump_down(command.jump);
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
            self.transport
                .send(ServerMessage::LightStyle { index, pattern }.encode());
        }
        for text in outbox.prints {
            self.transport.send(ServerMessage::Print(text).encode());
        }
        if let Some((yaw, pitch)) = outbox.view {
            self.transport
                .send(ServerMessage::SetView { yaw, pitch }.encode());
        }
        for sound in outbox.static_sounds {
            self.transport
                .send(ServerMessage::StaticSound(sound).encode());
        }
        for sound in outbox.sounds {
            self.transport.send(ServerMessage::Sound(sound).encode());
        }
        if let Some(track) = outbox.music {
            self.transport.send(ServerMessage::Music(track).encode());
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
            _ => {}
        }
    }
}
