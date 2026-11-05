//! A sky sphere: a gradient horizon, a sun and a layer of clouds (proposal
//! 0071).

use engine_core::math::Vec3;

use crate::material::SlotImage;

/// A layer of clouds on the sky, either an image or made up in the shader. The
/// author chooses.
#[derive(Clone, Debug, PartialEq)]
pub struct Clouds {
    /// Their colour.
    pub color: [f32; 3],
    /// How much of the sky they cover, from 0 to 1.
    pub coverage: f32,
    /// How they drift, in repeats a second.
    pub speed: [f32; 2],
    /// How large they are.
    pub scale: f32,
    /// A premade layer, or `None` to make them up.
    pub image: Option<SlotImage>,
}

/// A sky.
#[derive(Clone, Debug, PartialEq)]
pub struct Sky {
    /// The colour overhead.
    pub zenith: [f32; 3],
    /// The colour at the horizon.
    pub horizon: [f32; 3],
    /// The colour below it.
    pub ground: [f32; 3],
    /// How high the horizon's band reaches, from 0 to 1.
    pub horizon_height: f32,
    /// Towards the sun.
    pub sun_direction: Vec3,
    /// The sun's colour.
    pub sun_color: [f32; 3],
    /// How large the sun looks, as a cosine distance.
    pub sun_size: f32,
    /// The cloud layer, if there is one.
    pub clouds: Option<Clouds>,
}

impl Default for Sky {
    fn default() -> Self {
        Self {
            zenith: [0.16, 0.32, 0.62],
            horizon: [0.72, 0.8, 0.86],
            ground: [0.3, 0.3, 0.32],
            horizon_height: 0.25,
            sun_direction: Vec3::new(0.4, 0.3, 0.8).normalized(),
            sun_color: [1.0, 0.95, 0.8],
            sun_size: 0.002,
            clouds: None,
        }
    }
}
