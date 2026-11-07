//! Sprites, particles and lights, as a frame hands them over.

use engine_core::math::Vec3;
use engine_map::{
    EffectDraw, EmitterShape, GpuEmitter, ParticleCollide, ParticleEmitter, ParticleForce,
    ParticleKind, SimEmitter, SpriteFacing, SpriteImage,
};

use crate::matrix::rolled_axes;

/// Bytes of the values a sprite's draw pushes.
pub(crate) const SPRITE_PUSH_SIZE: u32 = 64;

/// Bytes in one particle.
pub(crate) const PARTICLE_BYTES: usize = 72;

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
    /// How it's turned, in radians.
    pub rotation: f32,
    /// How far it fades into what's behind it, in world units.
    pub soft: f32,
    /// Its picture, plus one, or 0 for a soft dot.
    pub picture: u32,
    /// Which frame of a sheet it's on, and how far to the next.
    pub frame: f32,
    /// The columns and rows of that sheet.
    pub sheet: [u32; 2],
    /// How far through its life it is.
    pub life: f32,
    /// The shader it has of its own, plus one, or 0 for none.
    pub shader: u32,
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
    /// Their picture, plus one, or 0 for a soft dot.
    pub picture: u32,
    /// How far they fade into what's behind them, in world units.
    pub soft: f32,
}

/// How many see-through surfaces a particle's looked for behind, the nearest ones.
const MOST_SEE_THROUGH: usize = 64;

/// Whether the line from `from` to `to` goes through the box from `low` to `high`.
fn line_meets_box(from: Vec3, to: Vec3, (low, high): (Vec3, Vec3)) -> bool {
    let (mut first, mut last) = (0.0_f32, 1.0_f32);
    let axes = [
        (from.x, to.x, low.x, high.x),
        (from.y, to.y, low.y, high.y),
        (from.z, to.z, low.z, high.z),
    ];
    for (start, end, bottom, top) in axes {
        let way = end - start;
        if way.abs() < f32::EPSILON {
            if start < bottom || start > top {
                return false;
            }
            continue;
        }
        let (enter, leave) = ((bottom - start) / way, (top - start) / way);
        first = first.max(enter.min(leave));
        last = last.min(enter.max(leave));
    }
    first <= last
}

/// `particles` with those behind a see-through surface from `eye` first, each lot in the order it
/// was in, and how many of them there are. Those go in ahead of the surface, so that it's over them.
pub(crate) fn behind_first<'a>(
    particles: &'a [ParticleDraw],
    eye: Vec3,
    boxes: &[(Vec3, Vec3)],
) -> (Vec<&'a ParticleDraw>, usize) {
    if boxes.is_empty() {
        return (particles.iter().collect(), 0);
    }
    let away = |(low, high): &(Vec3, Vec3)| ((*low + *high) * 0.5 - eye).length();
    let mut near: Vec<(Vec3, Vec3)> = boxes.to_vec();
    near.sort_by(|a, b| away(a).total_cmp(&away(b)));
    near.truncate(MOST_SEE_THROUGH);
    let (behind, rest): (Vec<&ParticleDraw>, Vec<&ParticleDraw>) =
        particles.iter().partition(|particle| {
            near.iter()
                .any(|bounds| line_meets_box(eye, particle.origin, *bounds))
        });
    let count = behind.len();
    (behind.into_iter().chain(rest).collect(), count)
}

/// Bytes in what a step of compute particles is told.
pub(crate) const SIM_EMITTER_BYTES: usize = 656;

/// A force on compute particles, as its kind, three numbers and two places.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SimForce {
    /// 0 for none, 1 attract, 2 vortex and 3 wind.
    pub kind: f32,
    /// How hard.
    pub strength: f32,
    /// Its reach, how hard it draws in, or how much it comes and goes.
    pub extra: f32,
    /// Where it is, from the effect's middle, or which way wind blows.
    pub at: Vec3,
    /// Which way a vortex's line goes.
    pub axis: Vec3,
}

/// An emitter compute runs, for one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParticleSim {
    /// Which it is, which goes on being it from frame to frame.
    pub id: u64,
    /// Where it is.
    pub origin: Vec3,
    /// How long this step is.
    pub seconds: f32,
    /// Which way they head.
    pub direction: Vec3,
    /// How far from that they can go, as a cosine.
    pub spread: f32,
    /// The slowest and fastest they start.
    pub speed: [f32; 2],
    /// The shortest and longest they live.
    pub life: [f32; 2],
    /// How far from `origin` they start.
    pub radius: f32,
    /// How hard gravity pulls.
    pub gravity: f32,
    /// How much they slow, as a share a second.
    pub drag: f32,
    /// How hard they're pushed about.
    pub turbulence: f32,
    /// How many slots it has.
    pub count: u32,
    /// How many start this step.
    pub spawn: u32,
    /// Their colour at eight even steps along a life.
    pub ramp: [[f32; 4]; 8],
    /// Their size at those steps.
    pub sizes: [f32; 8],
    /// Their picture, plus one, or 0 for a soft dot.
    pub picture: u32,
    /// The columns and rows of its sheet.
    pub sheet: [u32; 2],
    /// Its frames a second, or 0 to go through them once over a life.
    pub flip_rate: f32,
    /// How far they're drawn out along how they're moving, in seconds of it.
    pub stretch: f32,
    /// The least and most they start turned, in radians.
    pub rotation: [f32; 2],
    /// The least and most they turn a second, in radians.
    pub spin: [f32; 2],
    /// Whether they add to what's behind them.
    pub adds: bool,
    /// How far they fade into what's behind them.
    pub soft: f32,
    /// The time.
    pub time: f32,
    /// A number that makes this step differ from the last.
    pub seed: u32,
    /// How much speed they keep when they bounce.
    pub bounce: f32,
    /// What they do when they hit something: 0 nothing, 1 bounce, 2 die, 3 stick.
    pub collide: u32,
    /// Their shape: 0 a ball, 1 a ring, 2 a disc, 3 a cone, 4 a line, 5 a box.
    pub shape: u32,
    /// Half of a line or box, or a cone's height in z.
    pub extent: Vec3,
    /// Whether they head away from the middle of their shape.
    pub outward: bool,
    /// What pushes them.
    pub forces: [SimForce; 4],
    /// Which of the level's particle shaders they're drawn by, plus one, or 0 for none.
    pub shader: u32,
    /// Which of the level's motions they have, plus one, or 0 for none.
    pub motion: u32,
    /// Which of the level's fields they can read, plus one, or 0 for none.
    pub field: u32,
}

/// What a step of `sim` is told, with the view and the way back from it, where its slots are, and
/// whether they're new and there's depth to read.
pub(crate) fn sim_emitter_bytes(
    sim: &ParticleSim,
    (view, back): (&[f32; 16], &[f32; 16]),
    ring: [u32; 4],
    [fresh, depth_ok]: [u32; 2],
) -> [u8; SIM_EMITTER_BYTES] {
    let mut out = [0; SIM_EMITTER_BYTES];
    let mut at = 0;
    let mut put = |word: u32| {
        out[at..at + 4].copy_from_slice(&word.to_le_bytes());
        at += 4;
    };
    let floats = |list: &[f32], put: &mut dyn FnMut(u32)| {
        for number in list {
            put(number.to_bits());
        }
    };
    floats(view, &mut put);
    floats(back, &mut put);
    let (origin, way) = (sim.origin, sim.direction);
    floats(&[origin.x, origin.y, origin.z, sim.seconds], &mut put);
    floats(&[way.x, way.y, way.z, sim.spread], &mut put);
    floats(
        &[sim.speed[0], sim.speed[1], sim.life[0], sim.life[1]],
        &mut put,
    );
    floats(
        &[sim.radius, sim.gravity, sim.drag, sim.turbulence],
        &mut put,
    );
    for word in ring {
        put(word);
    }
    for color in &sim.ramp {
        floats(color, &mut put);
    }
    floats(&sim.sizes, &mut put);
    #[expect(clippy::cast_precision_loss, reason = "few pictures")]
    let picture = sim.picture as f32;
    floats(
        &[
            picture,
            f32::from(u8::from(sim.adds)),
            sim.flip_rate,
            sim.stretch,
        ],
        &mut put,
    );
    floats(&[sim.soft, sim.time], &mut put);
    put(sim.seed);
    floats(&[sim.bounce], &mut put);
    for word in [fresh, depth_ok, sim.collide, sim.shape] {
        put(word);
    }
    let reach = sim.extent;
    let outward = f32::from(u8::from(sim.outward));
    floats(&[reach.x, reach.y, reach.z, outward], &mut put);
    let [least, most] = sim.rotation;
    floats(&[least, most, sim.spin[0], sim.spin[1]], &mut put);
    #[expect(clippy::cast_precision_loss, reason = "few frames")]
    let sheet = [sim.sheet[0] as f32, sim.sheet[1] as f32, 0.0, 0.0];
    floats(&sheet, &mut put);
    for force in sim.forces {
        floats(&[force.kind, force.strength, force.extra, 0.0], &mut put);
        floats(&[force.at.x, force.at.y, force.at.z, 0.0], &mut put);
        floats(&[force.axis.x, force.axis.y, force.axis.z, 0.0], &mut put);
    }
    out
}

/// Bytes of what a draw of [`GpuParticles`] pushes.
pub(crate) const GPU_PARTICLE_PUSH: u32 = 112;

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
    #[expect(clippy::cast_precision_loss, reason = "few pictures")]
    let picture = particles.picture as f32;
    let adds = f32::from(u8::from(particles.adds));
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
        picture,
        adds,
        particles.soft,
        0.0,
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
    #[expect(clippy::cast_precision_loss, reason = "few pictures and frames")]
    let numbers = [
        particle.size,
        end.x,
        end.y,
        end.z,
        shape,
        particle.glow,
        f32::from(u8::from(particle.adds)),
        particle.rotation,
        particle.soft,
        particle.picture as f32,
        particle.frame,
        particle.sheet[0].max(1) as f32,
        particle.sheet[1].max(1) as f32,
        particle.life,
    ];
    if let Some(rest) = out.get_mut(16..PARTICLE_BYTES) {
        for (chunk, number) in rest.as_chunks_mut::<4>().0.iter_mut().zip(numbers) {
            *chunk = number.to_le_bytes();
        }
    }
}

/// How bright a colour in light is, past what a byte can hold.
fn glow_of(color: [f32; 4]) -> f32 {
    color[0].max(color[1]).max(color[2]).max(1.0)
}

/// A byte from a colour between 0 and 1, as the shader's curve expects.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "held to a byte"
)]
fn colour_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8
}

impl From<&EffectDraw> for ParticleDraw {
    fn from(draw: &EffectDraw) -> Self {
        let glow = glow_of(draw.color);
        let [red, green, blue, alpha] = draw.color;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "held to a byte"
        )]
        let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        Self {
            origin: draw.origin,
            color: ParticleColor {
                red: colour_byte(red / glow),
                green: colour_byte(green / glow),
                blue: colour_byte(blue / glow),
            },
            alpha,
            size: draw.size,
            end: draw.end,
            shape: match draw.kind {
                ParticleKind::Ribbon | ParticleKind::Beam => ParticleShape::Line,
                ParticleKind::Decal => ParticleShape::Flat,
                ParticleKind::Sprite | ParticleKind::Light => ParticleShape::Soft,
            },
            glow,
            adds: draw.adds,
            rotation: draw.rotation,
            soft: draw.soft,
            picture: draw.picture,
            frame: draw.frame,
            sheet: draw.sheet,
            life: draw.life,
            shader: draw.shader,
        }
    }
}

impl From<&GpuEmitter> for GpuParticles {
    fn from(emitter: &GpuEmitter) -> Self {
        Self {
            origin: emitter.origin,
            life: emitter.life,
            direction: emitter.direction,
            spread: emitter.spread,
            color: emitter.color,
            end_color: emitter.end_color,
            age: emitter.age,
            speed: emitter.speed,
            gravity: emitter.gravity,
            turbulence: emitter.turbulence,
            size: emitter.size,
            radius: emitter.radius,
            count: emitter.count,
            adds: emitter.adds,
            picture: emitter.picture,
            soft: emitter.soft,
        }
    }
}

impl ParticleSim {
    /// What's run of `emitter` this frame, with `light` on all of it.
    #[must_use]
    pub fn of(sim: &SimEmitter, emitter: &ParticleEmitter, light: [f32; 3]) -> Self {
        let mut forces = [SimForce::default(); 4];
        for (slot, force) in forces.iter_mut().zip(&emitter.forces) {
            *slot = match *force {
                ParticleForce::Attract {
                    at,
                    strength,
                    reach,
                } => SimForce {
                    kind: 1.0,
                    strength,
                    extra: reach,
                    at,
                    axis: Vec3::ZERO,
                },
                ParticleForce::Vortex {
                    at,
                    axis,
                    strength,
                    inward,
                } => SimForce {
                    kind: 2.0,
                    strength,
                    extra: inward,
                    at,
                    axis,
                },
                ParticleForce::Wind { blowing, gusts } => SimForce {
                    kind: 3.0,
                    strength: 0.0,
                    extra: gusts,
                    at: blowing,
                    axis: Vec3::ZERO,
                },
            };
        }
        let (shape, radius, extent) = match emitter.shape {
            EmitterShape::Ring(radius) => (1, radius, Vec3::ZERO),
            EmitterShape::Disc(radius) => (2, radius, Vec3::ZERO),
            EmitterShape::Cone(radius, height) => (3, radius, Vec3::new(0.0, 0.0, height)),
            EmitterShape::Line(half) => (4, 0.0, half),
            EmitterShape::Box(half) => (5, 0.0, half),
            EmitterShape::Ball(radius) => (0, radius, Vec3::ZERO),
            EmitterShape::Point => (0, 0.0, Vec3::ZERO),
        };
        let own = sim.settings;
        let step = |number: usize| {
            #[expect(clippy::cast_precision_loss, reason = "eight of them")]
            let at = number as f32 / 7.0;
            at
        };
        let ramp = std::array::from_fn(|number| {
            let [red, green, blue, alpha] = emitter.color.at(step(number));
            [
                red * own.color[0] * light[0],
                green * own.color[1] * light[1],
                blue * own.color[2] * light[2],
                alpha * sim.fade,
            ]
        });
        let sizes = std::array::from_fn(|number| emitter.size.at(step(number)) * own.size);
        Self {
            id: sim.id,
            origin: sim.origin,
            seconds: sim.seconds,
            direction: emitter.direction.normalized(),
            spread: emitter.spread.to_radians().cos(),
            speed: emitter.speed,
            life: emitter.life,
            radius,
            gravity: sim.gravity,
            drag: emitter.drag,
            turbulence: emitter.turbulence,
            count: sim.count,
            spawn: sim.spawn,
            ramp,
            sizes,
            picture: emitter.picture,
            sheet: emitter.flipbook,
            flip_rate: emitter.flip_rate,
            stretch: emitter.stretch,
            rotation: emitter.rotation.map(f32::to_radians),
            spin: emitter.spin.map(f32::to_radians),
            adds: emitter.blend == engine_map::ParticleBlend::Add,
            soft: emitter.soft,
            time: sim.time,
            seed: sim.seed,
            bounce: emitter.bounce,
            collide: match emitter.collide {
                ParticleCollide::Bounce => 1,
                ParticleCollide::Die => 2,
                ParticleCollide::Stick => 3,
                ParticleCollide::None => 0,
            },
            shape,
            extent,
            outward: emitter.outward,
            forces,
            shader: emitter.shader_number,
            motion: emitter.motion_number,
            field: emitter.field_number,
        }
    }
}
