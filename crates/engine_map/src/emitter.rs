//! Small fixed fires: a flame's light and sparks (proposal 0079).

use engine_core::math::Vec3;

/// A small fire. Its flame is drawn as faces, and this is what moves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    /// Where it is.
    pub origin: Vec3,
    /// How far its flickering light reaches.
    pub light_radius: f32,
    /// How many sparks it gives off a second.
    pub sparks: f32,
}
