//! What every system needs to know about what's in the world.

use crate::math::Vec3;

/// A brush model in the world.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModelInstance {
    /// The model's number in the map.
    pub model: u32,
    /// How far it has moved from where the map built it.
    pub origin: Vec3,
    /// Whether it's drawn.
    pub visible: bool,
    /// Whether it blocks movement.
    pub solid: bool,
}

/// An entity the client draws, by its model.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EntityState {
    /// Who it is, across snapshots.
    pub id: u64,
    /// Its model, by the number the server gave the name.
    pub model: u16,
    /// Which frame it shows.
    pub frame: u16,
    /// Which skin it shows.
    pub skin: u8,
    /// Effects, by the original's bits.
    pub effects: u8,
    /// Where it is.
    pub origin: Vec3,
    /// Its pitch, yaw and roll in degrees.
    pub angles: Vec3,
}
