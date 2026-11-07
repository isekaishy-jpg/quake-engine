//! A sky sphere: a gradient horizon, a sun and a layer of clouds (proposal
//! 0071).

use engine_core::math::Vec3;

use crate::material::SlotImage;

/// The most layers of clouds a sky has.
pub const MOST_CLOUDS: usize = 4;

/// How curved clouds are unless told, which is nearly flat overhead and comes down to the horizon.
pub const USUAL_CLOUD_CURVE: f32 = 0.03;

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
    /// How high they are in the world, which says how they go by as the view moves. At 0 they
    /// stay with the view.
    pub height: f32,
    /// How curved they are, from 0 for a flat plane to 1 for a dome.
    pub curve: f32,
    /// How much they take the sun's colour, and go dark where they're thick, from 0 to 1.
    pub lit: f32,
    /// A premade layer, or `None` to make them up.
    pub image: Option<SlotImage>,
}

impl Default for Clouds {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0],
            coverage: 0.5,
            speed: [0.01, 0.003],
            scale: 1.0,
            height: 0.0,
            curve: USUAL_CLOUD_CURVE,
            lit: 0.0,
            image: None,
        }
    }
}

/// Rays of light coming down through a sky.
#[derive(Clone, Debug, PartialEq)]
pub struct SkyRays {
    /// A picture of what lets light through, or `None` to go by the sky's own clouds.
    pub picture: Option<SlotImage>,
    /// How bright they are.
    pub strength: f32,
    /// Whether they come down into the level, past what's in the sky.
    pub into_level: bool,
    /// How wide a repeat of the picture is, in world units.
    pub size: f32,
    /// How the picture drifts, in repeats a second.
    pub speed: [f32; 2],
    /// How much the sun flares, looking right at it.
    pub flare: f32,
}

impl Default for SkyRays {
    fn default() -> Self {
        Self {
            picture: None,
            strength: 1.0,
            into_level: false,
            size: 2048.0,
            speed: [0.01, 0.003],
            flare: 1.0,
        }
    }
}

/// Fog through a map, thinning with height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    /// Its colour.
    pub color: [f32; 3],
    /// How thick it is at its base.
    pub density: f32,
    /// How high its base is.
    pub height: f32,
    /// How fast it thins going up.
    pub falloff: f32,
}

/// Fog in a place, with clear air beside it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FogVolume {
    /// Its lowest corner.
    pub low: Vec3,
    /// Its highest corner.
    pub high: Vec3,
    /// Its colour.
    pub color: [f32; 3],
    /// How thick it is.
    pub density: f32,
    /// How far in from its sides it takes to reach that.
    pub edge: f32,
    /// How much of it is wisps, from 0 for even fog to 1 for gaps between them.
    pub wisps: f32,
    /// How big wisps are.
    pub wisp_size: f32,
    /// How fast they go, and which way.
    pub drift: Vec3,
    /// Whether it takes the level's light, and that of what goes through it.
    pub lit: bool,
}

impl FogVolume {
    /// Whether it changes along a ray, so that it has to be gone through a step at a time.
    #[must_use]
    pub fn marched(&self) -> bool {
        self.wisps > 0.0 || self.lit || self.edge > 0.0
    }
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
    /// The layers of cloud, up to `MOST_CLOUDS`. The first is the lowest, so it's in front.
    pub clouds: Vec<Clouds>,
    /// Rays coming down through it, if an author wants them.
    pub rays: Option<SkyRays>,
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
            clouds: Vec::new(),
            rays: None,
        }
    }
}
