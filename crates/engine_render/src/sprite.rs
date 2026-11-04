//! Sprites, particles and lights, as a frame hands them over.

use engine_core::math::Vec3;
use engine_map::{SpriteFacing, SpriteImage};

/// Bytes of the values a sprite's draw pushes.
pub(crate) const SPRITE_PUSH_SIZE: u32 = 64;

/// Bytes in one particle.
pub(crate) const PARTICLE_BYTES: usize = 16;

/// The most particles a frame draws.
pub const MAX_PARTICLE_DRAWS: usize = 16_384;

/// The most dynamic lights a frame uses.
pub const MAX_LIGHT_DRAWS: usize = 32;

/// How close to straight up or down an upright sprite can't be drawn, as the
/// cosine of one degree.
const UPRIGHT_LIMIT: f32 = 0.999_848;

/// A sprite to draw this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpriteDraw {
    /// The number [`crate::Renderer::add_sprite`] gave it.
    pub sprite: u32,
    /// Which of its pictures.
    pub image: u32,
    /// Where it is.
    pub origin: Vec3,
    /// Its pitch, yaw and roll in degrees, for those that use them.
    pub angles: Vec3,
}

/// A particle to draw this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ParticleColor {
    /// Red.
    pub red: u8,
    /// Green.
    pub green: u8,
    /// Blue.
    pub blue: u8,
}

/// A particle to draw this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParticleDraw {
    /// Where it is.
    pub origin: Vec3,
    /// Its colour.
    pub color: ParticleColor,
}

/// A light shining this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LightDraw {
    /// Where it is.
    pub origin: Vec3,
    /// How far it reaches.
    pub radius: f32,
    /// The least it adds, which cuts off its edge.
    pub min_light: f32,
}

/// The directions a view looks along.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewAxes {
    /// Straight ahead.
    pub forward: Vec3,
    /// To the right.
    pub right: Vec3,
    /// Up.
    pub up: Vec3,
}

impl ViewAxes {
    /// The directions of `pitch`, `yaw` and `roll` in degrees, from
    /// `AngleVectors`, where a positive pitch looks down.
    #[must_use]
    pub fn new(pitch: f32, yaw: f32, roll: f32) -> Self {
        let (pitch_sin, pitch_cos) = pitch.to_radians().sin_cos();
        let (yaw_sin, yaw_cos) = yaw.to_radians().sin_cos();
        let (roll_sin, roll_cos) = roll.to_radians().sin_cos();
        Self {
            forward: Vec3::new(pitch_cos * yaw_cos, pitch_cos * yaw_sin, -pitch_sin),
            right: Vec3::new(
                -roll_sin * pitch_sin * yaw_cos + roll_cos * yaw_sin,
                -roll_sin * pitch_sin * yaw_sin - roll_cos * yaw_cos,
                -roll_sin * pitch_cos,
            ),
            up: Vec3::new(
                roll_cos * pitch_sin * yaw_cos + roll_sin * yaw_sin,
                roll_cos * pitch_sin * yaw_sin - roll_sin * yaw_cos,
                roll_cos * pitch_cos,
            ),
        }
    }
}

/// The right and up directions of a sprite, from `R_DrawSprite`, or `None` when
/// it can't be drawn from here. `to_sprite` runs from the eye to the sprite.
#[must_use]
pub fn sprite_axes(
    facing: SpriteFacing,
    view: &ViewAxes,
    to_sprite: Vec3,
    angles: Vec3,
) -> Option<(Vec3, Vec3)> {
    let upright = |along: Vec3| {
        (along.z.abs() <= UPRIGHT_LIMIT).then(|| {
            let right = Vec3::new(along.y, -along.x, 0.0).normalized();
            (right, Vec3::new(0.0, 0.0, 1.0))
        })
    };
    match facing {
        SpriteFacing::View => Some((view.right, view.up)),
        SpriteFacing::ViewUpright => upright(view.forward),
        SpriteFacing::FacingUpright => upright(to_sprite.normalized()),
        SpriteFacing::Oriented => {
            let own = ViewAxes::new(angles.x, angles.y, angles.z);
            Some((own.right, own.up))
        }
        SpriteFacing::ViewRolled => {
            let (sin, cos) = angles.z.to_radians().sin_cos();
            Some((
                view.right * cos + view.up * sin,
                view.right * -sin + view.up * cos,
            ))
        }
    }
}

/// The values a sprite's draw pushes.
#[expect(
    clippy::cast_precision_loss,
    reason = "a picture's size and place are far below 2^24"
)]
#[must_use]
pub fn sprite_push_bytes(draw: &SpriteDraw, image: &SpriteImage, axes: (Vec3, Vec3)) -> Vec<u8> {
    let (right, up) = axes;
    let values = [
        draw.origin.x,
        draw.origin.y,
        draw.origin.z,
        0.0, //
        right.x,
        right.y,
        right.z,
        image.left as f32, //
        up.x,
        up.y,
        up.z,
        image.top as f32, //
        image.width as f32,
        image.height as f32,
        0.0,
        0.0,
    ];
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// Appends a particle's bytes.
pub(crate) fn write_particle(particle: &ParticleDraw, out: &mut [u8]) {
    let origin = particle.origin;
    for (chunk, value) in out
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip([origin.x, origin.y, origin.z])
    {
        *chunk = value.to_le_bytes();
    }
    let color = particle.color;
    if let Some(end) = out.get_mut(12..PARTICLE_BYTES) {
        end.copy_from_slice(&[color.red, color.green, color.blue, 255]);
    }
}
