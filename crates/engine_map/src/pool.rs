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
    /// How much its surface mirrors, at a glance.
    pub reflect: f32,
    /// How tall its waves look.
    pub waves: f32,
    /// How far its surface bends what's seen through it.
    pub refraction: f32,
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
                reflect: 0.6,
                waves: 1.0,
                refraction: 1.0,
            }),
            Contents::Slime => Some(Self {
                color: [0.12, 0.3, 0.05],
                absorption: 0.02,
                caustics: 0.3,
                reflect: 0.25,
                waves: 0.5,
                refraction: 0.4,
            }),
            Contents::Lava => Some(Self {
                color: [0.75, 0.2, 0.03],
                absorption: 0.05,
                caustics: 0.0,
                reflect: 0.0,
                waves: 0.0,
                refraction: 0.0,
            }),
            _ => None,
        }
    }
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
