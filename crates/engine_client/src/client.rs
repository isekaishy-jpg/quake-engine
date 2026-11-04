//! The client: commands, prediction and the render tick (proposal 0024).

use std::collections::VecDeque;
use std::rc::Rc;

use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_movement::{MoveVars, player_move};
use engine_net::{ClientMessage, ServerMessage, Transport};
use engine_physics::CollisionWorld;

/// The most commands waiting for the server. Older ones are forgotten.
const MAX_PENDING: usize = 512;

/// The local player's side of a game.
pub struct Client {
    /// The connection to the server.
    transport: Box<dyn Transport>,
    /// The map's collision.
    world: Rc<CollisionWorld>,
    /// The rules of movement.
    vars: MoveVars,
    /// The next command's number.
    next_sequence: u32,
    /// Commands the server hasn't run yet.
    pending: VecDeque<UserCommand>,
    /// The player after every command so far.
    predicted: PlayerState,
}

impl Client {
    /// A client with its player at `origin`.
    #[must_use]
    pub fn new(transport: Box<dyn Transport>, world: Rc<CollisionWorld>, origin: Vec3) -> Self {
        Self {
            transport,
            world,
            vars: MoveVars::default(),
            next_sequence: 1,
            pending: VecDeque::new(),
            predicted: PlayerState {
                origin,
                ..PlayerState::default()
            },
        }
    }

    /// The player after every command so far.
    #[must_use]
    pub fn predicted(&self) -> &PlayerState {
        &self.predicted
    }

    /// How many commands the server hasn't run yet.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Numbers `command`, sends it, and predicts its result.
    pub fn send_command(&mut self, mut command: UserCommand, seconds: f32) {
        command.sequence = self.next_sequence;
        command.seconds = seconds;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        self.transport.send(ClientMessage::Move(command).encode());
        player_move(&mut self.predicted, &command, &self.vars, &self.world);
        if self.pending.len() == MAX_PENDING {
            let _oldest = self.pending.pop_front();
        }
        self.pending.push_back(command);
    }

    /// Sends a console command to the server.
    pub fn send_console(&mut self, text: &str) {
        self.transport
            .send(ClientMessage::Console(text.to_owned()).encode());
    }

    /// Reads what the server sent. Each snapshot resets the prediction to the
    /// server's state and replays what it hasn't run, as `CL_PredictMove`.
    pub fn receive(&mut self) {
        while let Some(bytes) = self.transport.receive() {
            let Some(ServerMessage::Snapshot(snapshot)) = ServerMessage::decode(&bytes) else {
                continue;
            };
            self.pending
                .retain(|command| command.sequence > snapshot.acknowledged);
            self.predicted = snapshot.player;
            for command in &self.pending {
                player_move(&mut self.predicted, command, &self.vars, &self.world);
            }
        }
    }

    /// The state to draw: a copy of the prediction, moved on by `seconds`
    /// with the latest input. The copy is thrown away, so the simulation
    /// stays on its fixed tick.
    #[must_use]
    pub fn render_state(&self, command: &UserCommand, seconds: f32) -> PlayerState {
        let mut state = self.predicted;
        if seconds > 0.0 {
            let mut command = *command;
            command.seconds = seconds;
            player_move(&mut state, &command, &self.vars, &self.world);
        }
        state
    }
}
