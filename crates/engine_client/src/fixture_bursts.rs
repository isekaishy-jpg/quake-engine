//! The sparks, smoke and blood of `r_part.c`, as an effect made in code from the palette
//! (proposals 0036 and 0146). How each moves and changes colour is an emitter. Where each starts
//! follows sums no emitter says, so that's here, and the particles are placed by hand.

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{ColorCurve, EffectDef, EffectSettings, ParticleEmitter, ParticleKind};

use crate::effect_system::{EffectSystem, PlacedParticle};

/// The colours of the first half of an explosion, brightest first.
const RAMP_EXPLODE: [u8; 8] = [0x6f, 0x6d, 0x6b, 0x69, 0x67, 0x65, 0x63, 0x61];

/// The colours of the second half of an explosion.
const RAMP_EXPLODE2: [u8; 8] = [0x6f, 0x6e, 0x6d, 0x6c, 0x6b, 0x6a, 0x68, 0x66];

/// The colours of fire turning to smoke.
const RAMP_FIRE: [u8; 6] = [0x6d, 0x6b, 6, 5, 4, 3];

/// Steps a second along the ramps.
const EXPLODE_STEPS: f32 = 10.0;
const EXPLODE2_STEPS: f32 = 15.0;
const FIRE_STEPS: f32 = 5.0;

/// A particle count that means an explosion.
const EXPLOSION_COUNT: u32 = 1024;

/// How much of gravity pulls on any of them.
const SLOW_GRAVITY: f32 = 0.05;

/// How fast those that speed up or slow down do, as a share a second.
const GROWTH: f32 = 4.0;

/// How long a spark off a small fire lasts, which is before the end of its ramp.
const SPARK_SECONDS: f32 = 1.0;

/// How many pieces each edge of an outline has.
const OUTLINE_STEPS: u8 = 6;

/// How long an outline stays, in seconds.
const OUTLINE_SECONDS: f32 = 2.0;

/// The emitters of the effect, by how each moves.
const STILL: usize = 0;
const FALL: usize = 1;
const FIRE: usize = 2;
const SPARK: usize = 3;
const EXPLODE: usize = 4;
const EXPLODE2: usize = 5;
const BLOB: usize = 6;
const BLOB2: usize = 7;

/// A trail an entity leaves, from `R_RocketTrail`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Trail {
    /// A rocket's fire.
    Rocket,
    /// A grenade's smoke.
    Smoke,
    /// A gib's blood.
    Blood,
    /// A little blood.
    SlightBlood,
    /// A wizard's green spit.
    Tracer,
    /// A death knight's orange spit.
    Tracer2,
    /// A vore's purple ball.
    Voor,
}

impl Trail {
    /// The distance between drops.
    fn spacing(self) -> f32 {
        if self == Self::SlightBlood { 6.0 } else { 3.0 }
    }
}

/// A colour of the palette, as effects have them.
fn in_light(color: [u8; 3]) -> [f32; 3] {
    color.map(|part| (f32::from(part) / 255.0).powf(2.2))
}

/// `ramp`'s colours, each for an even share of a life, with no mixing between.
fn stepped(ramp: &[u8], colors: &[[f32; 3]; 256]) -> ColorCurve {
    #[expect(clippy::cast_precision_loss, reason = "a ramp has few colours")]
    let steps = ramp.len() as f32;
    let mut stops = Vec::with_capacity(ramp.len() * 2);
    for (number, index) in (0_u16..).zip(ramp) {
        let [red, green, blue] = colors[usize::from(*index)];
        let from = f32::from(number) / steps;
        let to = f32::from(number + 1) / steps;
        stops.push((from, [red, green, blue, 1.0]));
        stops.push((to - 0.0001, [red, green, blue, 1.0]));
    }
    ColorCurve(stops)
}

/// The effect all of them belong to.
fn fixture_effect(colors: &[[f32; 3]; 256]) -> EffectDef {
    #[expect(clippy::cast_precision_loss, reason = "a ramp has few colours")]
    let lasting = |ramp: &[u8], steps: f32| ramp.len() as f32 / steps;
    let square = |name: &str| ParticleEmitter {
        name: name.to_owned(),
        kind: ParticleKind::Square,
        gravity: SLOW_GRAVITY,
        ..ParticleEmitter::default()
    };
    let fire_life = lasting(&RAMP_FIRE, FIRE_STEPS);
    let explode_life = lasting(&RAMP_EXPLODE, EXPLODE_STEPS);
    let explode2_life = lasting(&RAMP_EXPLODE2, EXPLODE2_STEPS);
    let emitters = vec![
        ParticleEmitter {
            gravity: 0.0,
            ..square("still")
        },
        square("fall"),
        ParticleEmitter {
            gravity: -SLOW_GRAVITY,
            life: [fire_life; 2],
            color: stepped(&RAMP_FIRE, colors),
            ..square("fire")
        },
        ParticleEmitter {
            gravity: -SLOW_GRAVITY,
            life: [SPARK_SECONDS; 2],
            color: stepped(&RAMP_FIRE[..5], colors),
            ..square("spark")
        },
        ParticleEmitter {
            life: [explode_life; 2],
            drag: -GROWTH,
            color: stepped(&RAMP_EXPLODE, colors),
            ..square("explode")
        },
        ParticleEmitter {
            life: [explode2_life; 2],
            drag: 1.0,
            color: stepped(&RAMP_EXPLODE2, colors),
            ..square("explode2")
        },
        ParticleEmitter {
            drag: -GROWTH,
            ..square("blob")
        },
        ParticleEmitter {
            flat_drag: GROWTH,
            ..square("blob2")
        },
    ];
    EffectDef {
        name: "fixture".to_owned(),
        emitters,
        timeline: Vec::new(),
    }
}

/// What starts the fixture's particles.
#[derive(Clone, Debug)]
pub(crate) struct Bursts {
    /// What they are.
    effect: Arc<EffectDef>,
    /// The palette, in light.
    colors: Box<[[f32; 3]; 256]>,
    /// The state of `rand`.
    seed: u32,
    /// How many tracers have been dropped, which sets their colour and side.
    tracers: u32,
}

impl Default for Bursts {
    fn default() -> Self {
        Self::new(&|index| [index; 3])
    }
}

impl Bursts {
    /// Those of the palette `color` says.
    fn new(color: &dyn Fn(u8) -> [u8; 3]) -> Self {
        let mut colors = Box::new([[0.0; 3]; 256]);
        for (slot, index) in colors.iter_mut().zip(0_u8..=255) {
            *slot = in_light(color(index));
        }
        Self {
            effect: Arc::new(fixture_effect(&colors)),
            colors,
            seed: 1,
            tracers: 0,
        }
    }

    /// Uses the palette `color` says, and goes on from where `rand` was.
    pub(crate) fn set_palette(&mut self, color: &dyn Fn(u8) -> [u8; 3]) {
        let seed = self.seed;
        *self = Self::new(color);
        self.seed = seed;
    }

    /// A number from 0 to 32767, as C's `rand`.
    pub(crate) fn rand(&mut self) -> u32 {
        self.seed = self.seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.seed >> 16) & 0x7fff
    }

    /// A number from 0 up to `limit`, as a float.
    #[expect(clippy::cast_precision_loss, reason = "the numbers are all below 2^15")]
    fn below(&mut self, limit: u32) -> f32 {
        (self.rand() % limit) as f32
    }

    /// A vector of three numbers from 0 up to `limit`, moved down by `shift`.
    fn spread(&mut self, limit: u32, shift: f32) -> Vec3 {
        Vec3::new(
            self.below(limit) - shift,
            self.below(limit) - shift,
            self.below(limit) - shift,
        )
    }

    /// Starts one of `emitter`.
    fn add(&self, system: &mut EffectSystem, emitter: usize, placed: PlacedParticle) {
        system.place(&self.effect, emitter, placed);
    }

    /// What one of palette colour `index` is told of itself, to go on for `life` seconds.
    fn plain(&self, index: u8, life: f32, origin: Vec3, velocity: Vec3) -> PlacedParticle {
        PlacedParticle {
            origin,
            velocity,
            life: Some(life),
            settings: EffectSettings {
                color: self.colors[usize::from(index)],
                ..EffectSettings::default()
            },
            ..PlacedParticle::default()
        }
    }

    /// `R_ParticleExplosion`.
    pub(crate) fn explosion(&mut self, system: &mut EffectSystem, origin: Vec3) {
        for index in 0..EXPLOSION_COUNT {
            let (emitter, steps) = if index % 2 == 1 {
                (EXPLODE, EXPLODE_STEPS)
            } else {
                (EXPLODE2, EXPLODE2_STEPS)
            };
            let origin = origin + self.spread(32, 16.0);
            let velocity = self.spread(512, 256.0);
            let age = self.below(4) / steps;
            let placed = PlacedParticle {
                origin,
                velocity,
                age,
                ..PlacedParticle::default()
            };
            self.add(system, emitter, placed);
        }
    }

    /// `R_BlobExplosion`, for a spawn going off.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the colours are all below 256"
    )]
    pub(crate) fn blob_explosion(&mut self, system: &mut EffectSystem, origin: Vec3) {
        for index in 0..EXPLOSION_COUNT {
            let (emitter, color) = if index % 2 == 1 {
                (BLOB, 66 + self.rand() % 6)
            } else {
                (BLOB2, 150 + self.rand() % 6)
            };
            let extra = (self.rand() & 8) as f32 * 0.05;
            let origin = origin + self.spread(32, 16.0);
            let velocity = self.spread(512, 256.0);
            let placed = self.plain(color as u8, 1.0 + extra, origin, velocity);
            self.add(system, emitter, placed);
        }
    }

    /// A spark rising off a small fire at `origin`.
    pub(crate) fn spark(&mut self, system: &mut EffectSystem, origin: Vec3) {
        let origin = origin + self.spread(8, 1.0);
        let velocity = self.spread(16, 1.0) + Vec3::new(0.0, 0.0, 28.0);
        let placed = PlacedParticle {
            origin,
            velocity,
            ..PlacedParticle::default()
        };
        self.add(system, SPARK, placed);
    }

    /// `R_RunParticleEffect`: a puff around `origin`, drifting along `direction`.
    pub(crate) fn puff(
        &mut self,
        system: &mut EffectSystem,
        origin: Vec3,
        direction: Vec3,
        (color, count): (u8, u32),
    ) {
        if count >= EXPLOSION_COUNT {
            self.explosion(system, origin);
            return;
        }
        for _ in 0..count {
            let shade = (self.rand() & 7) as u8;
            let life = 0.1 * self.below(5);
            let origin = origin + self.spread(16, 8.0);
            let placed = self.plain((color & !7) | shade, life, origin, direction * 15.0);
            self.add(system, FALL, placed);
        }
    }

    /// Marks the edges of a box from `low` to `high` for a moment, in `color`.
    pub(crate) fn outline(&self, system: &mut EffectSystem, low: Vec3, high: Vec3, color: u8) {
        let corner = |bits: u8| {
            Vec3::new(
                if bits & 1 == 0 { low.x } else { high.x },
                if bits & 2 == 0 { low.y } else { high.y },
                if bits & 4 == 0 { low.z } else { high.z },
            )
        };
        for from in 0..8u8 {
            for axis in [1u8, 2, 4] {
                if from & axis != 0 {
                    continue;
                }
                let (start, end) = (corner(from), corner(from | axis));
                for step in 0..=OUTLINE_STEPS {
                    let share = f32::from(step) / f32::from(OUTLINE_STEPS);
                    let at = start.lerp(end, share);
                    let placed = self.plain(color, OUTLINE_SECONDS, at, Vec3::ZERO);
                    self.add(system, STILL, placed);
                }
            }
        }
    }

    /// `R_LavaSplash`, for the end of the game.
    #[expect(
        clippy::cast_precision_loss,
        reason = "the numbers are all far below 2^15"
    )]
    pub(crate) fn lava_splash(&mut self, system: &mut EffectSystem, origin: Vec3) {
        for row in -16_i32..16 {
            for column in -16_i32..16 {
                let direction = Vec3::new(
                    (column * 8) as f32 + self.below(8),
                    (row * 8) as f32 + self.below(8),
                    256.0,
                );
                let speed = 50.0 + self.below(64);
                let life = 2.0 + self.below(32) * 0.02;
                let color = 224 + (self.rand() & 7) as u8;
                let height = self.below(64);
                let at = origin + Vec3::new(direction.x, direction.y, height);
                let placed = self.plain(color, life, at, direction.normalized() * speed);
                self.add(system, FALL, placed);
            }
        }
    }

    /// `R_TeleportSplash`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "the numbers are all far below 2^15"
    )]
    pub(crate) fn teleport_splash(&mut self, system: &mut EffectSystem, origin: Vec3) {
        for row in (-16_i32..16).step_by(4) {
            for column in (-16_i32..16).step_by(4) {
                for level in (-24_i32..32).step_by(4) {
                    let place = Vec3::new(row as f32, column as f32, level as f32);
                    let direction = Vec3::new(column as f32, row as f32, level as f32);
                    let speed = 50.0 + self.below(64);
                    let life = 0.2 + self.below(8) * 0.02;
                    let color = 7 + (self.rand() & 7) as u8;
                    let at = origin + place + self.spread(4, 0.0);
                    let placed = self.plain(color, life, at, direction.normalized() * speed);
                    self.add(system, FALL, placed);
                }
            }
        }
    }

    /// `R_RocketTrail`: drops particles from `start` to `end`. `carried` holds how far it has gone
    /// since the last drop, so the spacing doesn't depend on the frame rate.
    pub(crate) fn trail(
        &mut self,
        system: &mut EffectSystem,
        (start, end): (Vec3, Vec3),
        kind: Trail,
        carried: &mut f32,
    ) {
        let path = end - start;
        let length = path.length();
        if length <= 0.0 {
            return;
        }
        let direction = path.normalized();
        let spacing = kind.spacing();
        let mut along = spacing - *carried;
        while along <= length {
            self.drop_trail(system, start + direction * along, direction, kind);
            along += spacing;
        }
        *carried = length - (along - spacing);
    }

    /// One drop of a trail.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a tracer's colour is a small number"
    )]
    fn drop_trail(&mut self, system: &mut EffectSystem, at: Vec3, direction: Vec3, kind: Trail) {
        match kind {
            Trail::Rocket | Trail::Smoke => {
                let offset = if kind == Trail::Rocket { 0.0 } else { 2.0 };
                let age = (self.below(4) + offset) / FIRE_STEPS;
                let origin = at + self.spread(6, 3.0);
                let placed = PlacedParticle {
                    origin,
                    age,
                    ..PlacedParticle::default()
                };
                self.add(system, FIRE, placed);
            }
            Trail::Blood | Trail::SlightBlood => {
                let color = 67 + (self.rand() & 3) as u8;
                let origin = at + self.spread(6, 3.0);
                let placed = self.plain(color, 2.0, origin, Vec3::ZERO);
                self.add(system, FALL, placed);
            }
            Trail::Tracer | Trail::Tracer2 => {
                let base = if kind == Trail::Tracer { 52 } else { 230 };
                let color = base + ((self.tracers & 4) << 1) as u8;
                self.tracers = self.tracers.wrapping_add(1);
                let side = if self.tracers % 2 == 1 { 30.0 } else { -30.0 };
                let velocity = Vec3::new(side * direction.y, -side * direction.x, 0.0);
                let placed = self.plain(color, 0.5, at, velocity);
                self.add(system, STILL, placed);
            }
            Trail::Voor => {
                let color = 9 * 16 + 8 + (self.rand() & 3) as u8;
                let origin = at + self.spread(16, 8.0);
                let placed = self.plain(color, 0.3, origin, Vec3::ZERO);
                self.add(system, STILL, placed);
            }
        }
    }
}
