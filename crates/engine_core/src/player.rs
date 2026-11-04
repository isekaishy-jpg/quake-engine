//! A player's state and input, as plain values that any system can carry.

use crate::math::Vec3;

/// What liquid a player is in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Liquid {
    /// None.
    #[default]
    None,
    /// Water.
    Water,
    /// Slime.
    Slime,
    /// Lava.
    Lava,
}

/// Everything that decides how a player moves next (proposal 0023).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerState {
    /// The centre of the player's box.
    pub origin: Vec3,
    /// Units a second.
    pub velocity: Vec3,
    /// Whether the player is standing on something.
    pub on_ground: bool,
    /// How deep the player is, from 0 for dry to 3 for under.
    pub water_level: u8,
    /// The liquid the player is in.
    pub liquid: Liquid,
    /// Seconds left of a jump out of water.
    pub water_jump_time: f32,
    /// Whether jump was held last time, so holding it doesn't repeat.
    pub jump_held: bool,
    /// Whether the player passes through everything.
    pub noclip: bool,
}

/// One numbered slice of player input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct UserCommand {
    /// This command's number, rising by one each time.
    pub sequence: u32,
    /// How long this command lasts, in seconds.
    pub seconds: f32,
    /// The view's yaw in degrees.
    pub yaw: f32,
    /// The view's pitch in degrees. Positive looks down.
    pub pitch: f32,
    /// Wanted speed forward.
    pub forward: f32,
    /// Wanted speed to the right.
    pub side: f32,
    /// Wanted speed up.
    pub up: f32,
    /// Whether jump is held.
    pub jump: bool,
}
