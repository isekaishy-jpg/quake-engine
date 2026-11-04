//! The server-side simulation.
//!
//! For now it's the one player. Entities join it in increment 4.
//!
//! May depend on `engine_core`, `engine_map`, `engine_physics`, `engine_movement`. See proposal 0008.

use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_movement::{MoveVars, player_move};
use engine_physics::CollisionWorld;

/// The longest a single command may claim to last, in seconds, so a client
/// can't move further than this at once.
const MAX_COMMAND_SECONDS: f32 = 0.1;

/// The game's state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Simulation {
    /// The player.
    player: PlayerState,
    /// The rules of movement.
    vars: MoveVars,
    /// The last command run.
    last_command: u32,
}

impl Simulation {
    /// A simulation with the player at `origin`.
    #[must_use]
    pub fn new(origin: Vec3) -> Self {
        Self {
            player: PlayerState {
                origin,
                ..PlayerState::default()
            },
            ..Self::default()
        }
    }

    /// The player.
    #[must_use]
    pub fn player(&self) -> &PlayerState {
        &self.player
    }

    /// The last command run.
    #[must_use]
    pub fn last_command(&self) -> u32 {
        self.last_command
    }

    /// Runs `command`, unless it's older than one already run.
    pub fn run_command(&mut self, command: &UserCommand, world: &CollisionWorld) {
        if command.sequence <= self.last_command {
            return;
        }
        self.last_command = command.sequence;
        let mut command = *command;
        command.seconds = command.seconds.clamp(0.0, MAX_COMMAND_SECONDS);
        player_move(&mut self.player, &command, &self.vars, world);
    }

    /// Turns noclip on or off, and returns the new setting.
    pub fn toggle_noclip(&mut self) -> bool {
        self.player.noclip = !self.player.noclip;
        self.player.noclip
    }

    /// Moves the player to `origin`, at rest.
    pub fn teleport(&mut self, origin: Vec3) {
        self.player.origin = origin;
        self.player.velocity = Vec3::ZERO;
    }
}
