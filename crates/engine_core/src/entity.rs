//! What every system needs to know about a placed brush model.

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
