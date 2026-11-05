//! Bodies of liquid, for the light in them (proposal 0077).

use engine_core::math::Vec3;

use crate::tree::Contents;

/// How a body of liquid looks from inside.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoolLook {
    /// The colour depth takes things towards.
    pub color: [f32; 3],
    /// How fast depth tints, per unit.
    pub absorption: f32,
    /// How strong caustics and shafts are.
    pub caustics: f32,
}

impl PoolLook {
    /// The usual look of `contents`, if it's a liquid.
    #[must_use]
    pub const fn of(contents: Contents) -> Option<Self> {
        match contents {
            Contents::Water => Some(Self {
                color: [0.05, 0.22, 0.28],
                absorption: 0.006,
                caustics: 0.6,
            }),
            Contents::Slime => Some(Self {
                color: [0.12, 0.3, 0.05],
                absorption: 0.02,
                caustics: 0.3,
            }),
            Contents::Lava => Some(Self {
                color: [0.75, 0.2, 0.03],
                absorption: 0.05,
                caustics: 0.0,
            }),
            _ => None,
        }
    }
}

/// A ball of a moving thing's shade.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OccluderSphere {
    /// Its middle, in the model's own space.
    pub center: [f32; 3],
    /// How big it is.
    pub radius: f32,
}

/// The shade a brush model casts wherever it goes. The first sphere is the
/// whole of it, and the others are its parts, for up close.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelOccluder {
    /// The model.
    pub model: u32,
    /// Its spheres.
    pub spheres: Vec<OccluderSphere>,
}

/// A body of liquid, as a box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pool {
    /// Its lowest corner.
    pub low: Vec3,
    /// Its highest corner.
    pub high: Vec3,
    /// How it looks.
    pub look: PoolLook,
}
