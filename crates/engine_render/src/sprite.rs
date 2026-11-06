//! Sprites, particles and lights, as a frame hands them over.

use engine_core::math::Vec3;
use engine_map::{SpriteFacing, SpriteImage};

use crate::matrix::rolled_axes;

/// Bytes of the values a sprite's draw pushes.
pub(crate) const SPRITE_PUSH_SIZE: u32 = 64;

/// Bytes in one particle.
pub(crate) const PARTICLE_BYTES: usize = 40;

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

/// What a particle looks like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ParticleShape {
    /// A small square that shrinks to a limit, as the original's are.
    #[default]
    Square,
    /// A soft round picture facing the eye.
    Soft,
    /// A line to `end`.
    Line,
    /// A flat mark facing `end`.
    Flat,
}

/// A particle to draw this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParticleDraw {
    /// Where it is.
    pub origin: Vec3,
    /// Its colour.
    pub color: ParticleColor,
    /// Its alpha.
    pub alpha: u8,
    /// Its size, for all but a square.
    pub size: f32,
    /// Where a line ends, or which way a flat mark faces.
    pub end: Vec3,
    /// What it looks like.
    pub shape: ParticleShape,
    /// How far past its colour it glows.
    pub glow: f32,
    /// Whether it adds to what's behind it.
    pub adds: bool,
}

/// Particles the GPU works out, which it does from these and nothing else.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GpuParticles {
    /// Where they start.
    pub origin: Vec3,
    /// How long each lives.
    pub life: f32,
    /// Which way they head.
    pub direction: Vec3,
    /// How far from that they can go, as a cosine.
    pub spread: f32,
    /// Their colour at the start.
    pub color: [f32; 4],
    /// Their colour at the end.
    pub end_color: [f32; 4],
    /// How long they've been going.
    pub age: f32,
    /// How fast they start.
    pub speed: f32,
    /// How hard gravity pulls.
    pub gravity: f32,
    /// How hard they're pushed about.
    pub turbulence: f32,
    /// Their size at the start and the end.
    pub size: [f32; 2],
    /// How far from `origin` they start.
    pub radius: f32,
    /// How many there are.
    pub count: u32,
    /// Whether they add to what's behind them.
    pub adds: bool,
}

/// Bytes of what a draw of [`GpuParticles`] pushes.
pub(crate) const GPU_PARTICLE_PUSH: u32 = 96;

/// The most draws of [`GpuParticles`] in a frame.
pub const MAX_GPU_PARTICLE_DRAWS: usize = 64;

/// What's pushed for `particles`.
pub(crate) fn gpu_particle_bytes(particles: &GpuParticles) -> [u8; GPU_PARTICLE_PUSH as usize] {
    let [ox, oy, oz] = [particles.origin.x, particles.origin.y, particles.origin.z];
    let [dx, dy, dz] = [
        particles.direction.x,
        particles.direction.y,
        particles.direction.z,
    ];
    let [r, g, b, a] = particles.color;
    let [er, eg, eb, ea] = particles.end_color;
    #[expect(clippy::cast_precision_loss, reason = "counts are far below 2^24")]
    let count = particles.count as f32;
    let numbers = [
        ox,
        oy,
        oz,
        particles.life,
        dx,
        dy,
        dz,
        particles.spread,
        r,
        g,
        b,
        a,
        er,
        eg,
        eb,
        ea,
        particles.age,
        particles.speed,
        particles.gravity,
        particles.turbulence,
        particles.size[0],
        particles.size[1],
        particles.radius,
        count,
    ];
    let mut out = [0; GPU_PARTICLE_PUSH as usize];
    for (chunk, number) in out.as_chunks_mut::<4>().0.iter_mut().zip(numbers) {
        *chunk = number.to_le_bytes();
    }
    out
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
        let [forward, right, up] = rolled_axes(yaw, pitch, roll);
        Self { forward, right, up }
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
    let alpha = match particle.shape {
        ParticleShape::Square => 255,
        _ => particle.alpha,
    };
    if let Some(end) = out.get_mut(12..16) {
        end.copy_from_slice(&[color.red, color.green, color.blue, alpha]);
    }
    let shape: f32 = match particle.shape {
        ParticleShape::Square => 0.0,
        ParticleShape::Soft => 1.0,
        ParticleShape::Line => 2.0,
        ParticleShape::Flat => 3.0,
    };
    let end = particle.end;
    let numbers = [particle.size, end.x, end.y, end.z, shape, particle.glow];
    if let Some(rest) = out.get_mut(16..PARTICLE_BYTES) {
        for (chunk, number) in rest.as_chunks_mut::<4>().0.iter_mut().zip(numbers) {
            *chunk = number.to_le_bytes();
        }
    }
}
