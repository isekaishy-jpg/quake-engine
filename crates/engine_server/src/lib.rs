//! The authoritative game server.
//!
//! It only hears from clients through a transport, even in a local game
//! (proposal 0024).
//!
//! May depend on `engine_core`, `engine_console`, `engine_map`, `engine_physics`, `engine_sim`, `engine_script`, `engine_net`. See proposal 0008.

use std::rc::Rc;

use engine_core::math::Vec3;
use engine_net::{ClientMessage, ServerMessage, Snapshot, Transport};
use engine_physics::CollisionWorld;
use engine_sim::Simulation;

/// A running game.
pub struct Server {
    /// The connection to the client.
    transport: Box<dyn Transport>,
    /// The map's collision.
    world: Rc<CollisionWorld>,
    /// The game.
    simulation: Simulation,
    /// Ticks run.
    tick: u32,
}

impl Server {
    /// A game with its player at `origin`.
    #[must_use]
    pub fn new(transport: Box<dyn Transport>, world: Rc<CollisionWorld>, origin: Vec3) -> Self {
        Self {
            transport,
            world,
            simulation: Simulation::new(origin),
            tick: 0,
        }
    }

    /// Runs what the client asked for, and tells it the result.
    pub fn tick(&mut self) {
        while let Some(bytes) = self.transport.receive() {
            match ClientMessage::decode(&bytes) {
                Some(ClientMessage::Move(command)) => {
                    self.simulation.run_command(&command, &self.world);
                }
                Some(ClientMessage::Console(text)) => self.console(&text),
                None => {}
            }
        }
        self.tick = self.tick.wrapping_add(1);
        let snapshot = Snapshot {
            tick: self.tick,
            acknowledged: self.simulation.last_command(),
            player: *self.simulation.player(),
        };
        self.transport
            .send(ServerMessage::Snapshot(snapshot).encode());
    }

    /// Runs a console command from the client.
    fn console(&mut self, text: &str) {
        let mut words = text.split_whitespace();
        match words.next() {
            Some("noclip") => {
                let _state = self.simulation.toggle_noclip();
            }
            Some("setpos") => {
                let mut number = || words.next().and_then(|word| word.parse::<f32>().ok());
                if let (Some(x), Some(y), Some(z)) = (number(), number(), number()) {
                    self.simulation.teleport(Vec3::new(x, y, z));
                }
            }
            _ => {}
        }
    }
}
