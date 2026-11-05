//! Particles: the sparks, smoke and blood of `r_part.c` (proposal 0036).

use engine_core::math::Vec3;

/// The most particles alive at once. The original's 2048 run out after two
/// rockets.
const MAX_PARTICLES: usize = 16_384;

/// How fast things fall, as `sv_gravity`.
const USUAL_GRAVITY: f32 = 800.0;

/// The colours of the first half of an explosion, brightest first.
const RAMP_EXPLODE: [u8; 8] = [0x6f, 0x6d, 0x6b, 0x69, 0x67, 0x65, 0x63, 0x61];

/// The colours of the second half of an explosion.
const RAMP_EXPLODE2: [u8; 8] = [0x6f, 0x6e, 0x6d, 0x6c, 0x6b, 0x6a, 0x68, 0x66];

/// The colours of fire turning to smoke.
const RAMP_FIRE: [u8; 6] = [0x6d, 0x6b, 6, 5, 4, 3];

/// A particle count that means an explosion.
const EXPLOSION_COUNT: u32 = 1024;

/// How a particle moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Motion {
    /// In a straight line.
    Still,
    /// Rises through the fire ramp.
    Fire,
    /// Speeds up through the first explosion ramp.
    Explode,
    /// Slows down through the second explosion ramp.
    Explode2,
    /// Speeds up, and sinks.
    Blob,
    /// Slows down sideways, and sinks.
    Blob2,
    /// Falls gently.
    Fall,
}

/// One particle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    /// Where it is.
    pub origin: Vec3,
    /// Its palette colour.
    pub color: u8,
    /// Units a second.
    velocity: Vec3,
    /// How far along its colour ramp it is.
    ramp: f32,
    /// When it goes.
    die: f32,
    /// How it moves.
    motion: Motion,
}

/// A trail an entity leaves, from `R_RocketTrail`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trail {
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

/// How many pieces each edge of an outline has.
const OUTLINE_STEPS: u8 = 6;

/// How long an outline stays, in seconds.
const OUTLINE_SECONDS: f32 = 2.0;

/// Every live particle.
#[derive(Clone, Debug)]
pub struct Particles {
    /// The particles.
    live: Vec<Particle>,
    /// The state of `rand`.
    seed: u32,
    /// How many tracers have been dropped, which sets their colour and side.
    tracers: u32,
    /// Seconds on the same clock as [`Particles::update`].
    time: f32,
    /// `sv_gravity`.
    gravity: f32,
}

impl Default for Particles {
    fn default() -> Self {
        Self {
            live: Vec::new(),
            seed: 1,
            tracers: 0,
            time: 0.0,
            gravity: USUAL_GRAVITY,
        }
    }
}

impl Particles {
    /// Sets what particles fall by.
    pub fn set_gravity(&mut self, gravity: f32) {
        self.gravity = gravity;
    }

    /// The particles to draw.
    #[must_use]
    pub fn live(&self) -> &[Particle] {
        &self.live
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

    /// Adds `particle`, unless there are too many.
    fn add(&mut self, particle: Particle) {
        if self.live.len() < MAX_PARTICLES {
            self.live.push(particle);
        }
    }

    /// `R_ParticleExplosion`.
    pub fn explosion(&mut self, origin: Vec3) {
        for index in 0..EXPLOSION_COUNT {
            let motion = if index % 2 == 1 {
                Motion::Explode
            } else {
                Motion::Explode2
            };
            let particle = Particle {
                origin: origin + self.spread(32, 16.0),
                color: RAMP_EXPLODE[0],
                velocity: self.spread(512, 256.0),
                ramp: self.below(4),
                die: self.time + 5.0,
                motion,
            };
            self.add(particle);
        }
    }

    /// `R_BlobExplosion`, for a spawn going off.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the colours are all below 256"
    )]
    pub fn blob_explosion(&mut self, origin: Vec3) {
        for index in 0..EXPLOSION_COUNT {
            let (motion, color) = if index % 2 == 1 {
                (Motion::Blob, 66 + self.rand() % 6)
            } else {
                (Motion::Blob2, 150 + self.rand() % 6)
            };
            let extra = (self.rand() & 8) as f32 * 0.05;
            let particle = Particle {
                origin: origin + self.spread(32, 16.0),
                color: color as u8,
                velocity: self.spread(512, 256.0),
                ramp: 0.0,
                die: self.time + 1.0 + extra,
                motion,
            };
            self.add(particle);
        }
    }

    /// `R_RunParticleEffect`: a puff around `origin`, drifting along `direction`.
    pub fn puff(&mut self, origin: Vec3, direction: Vec3, color: u8, count: u32) {
        if count >= EXPLOSION_COUNT {
            self.explosion(origin);
            return;
        }
        for _ in 0..count {
            let shade = (self.rand() & 7) as u8;
            let life = 0.1 * self.below(5);
            let particle = Particle {
                origin: origin + self.spread(16, 8.0),
                color: (color & !7) | shade,
                velocity: direction * 15.0,
                ramp: 0.0,
                die: self.time + life,
                motion: Motion::Fall,
            };
            self.add(particle);
        }
    }

    /// Marks the edges of a box from `low` to `high` for a moment, in `color`.
    pub fn outline(&mut self, low: Vec3, high: Vec3, color: u8) {
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
                    self.add(Particle {
                        origin: start.lerp(end, share),
                        color,
                        velocity: Vec3::ZERO,
                        ramp: 0.0,
                        die: self.time + OUTLINE_SECONDS,
                        motion: Motion::Still,
                    });
                }
            }
        }
    }

    /// `R_LavaSplash`, for the end of the game.
    #[expect(
        clippy::cast_precision_loss,
        reason = "the numbers are all far below 2^15"
    )]
    pub fn lava_splash(&mut self, origin: Vec3) {
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
                self.add(Particle {
                    origin: origin + Vec3::new(direction.x, direction.y, height),
                    color,
                    velocity: direction.normalized() * speed,
                    ramp: 0.0,
                    die: self.time + life,
                    motion: Motion::Fall,
                });
            }
        }
    }

    /// `R_TeleportSplash`.
    #[expect(
        clippy::cast_precision_loss,
        reason = "the numbers are all far below 2^15"
    )]
    pub fn teleport_splash(&mut self, origin: Vec3) {
        for row in (-16_i32..16).step_by(4) {
            for column in (-16_i32..16).step_by(4) {
                for level in (-24_i32..32).step_by(4) {
                    let place = Vec3::new(row as f32, column as f32, level as f32);
                    let direction = Vec3::new(column as f32, row as f32, level as f32);
                    let speed = 50.0 + self.below(64);
                    let life = 0.2 + self.below(8) * 0.02;
                    let color = 7 + (self.rand() & 7) as u8;
                    let particle = Particle {
                        origin: origin + place + self.spread(4, 0.0),
                        color,
                        velocity: direction.normalized() * speed,
                        ramp: 0.0,
                        die: self.time + life,
                        motion: Motion::Fall,
                    };
                    self.add(particle);
                }
            }
        }
    }

    /// `R_RocketTrail`: drops particles from `start` to `end`. `carried` holds
    /// how far it has gone since the last drop, so the spacing doesn't depend
    /// on the frame rate.
    pub fn trail(&mut self, start: Vec3, end: Vec3, kind: Trail, carried: &mut f32) {
        let path = end - start;
        let length = path.length();
        if length <= 0.0 {
            return;
        }
        let direction = path.normalized();
        let spacing = kind.spacing();
        let mut along = spacing - *carried;
        while along <= length {
            self.drop_trail(start + direction * along, direction, kind);
            along += spacing;
        }
        *carried = length - (along - spacing);
    }

    /// One drop of a trail.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the ramp is a small positive number"
    )]
    fn drop_trail(&mut self, at: Vec3, direction: Vec3, kind: Trail) {
        let mut particle = Particle {
            origin: at,
            color: 0,
            velocity: Vec3::ZERO,
            ramp: 0.0,
            die: self.time + 2.0,
            motion: Motion::Still,
        };
        match kind {
            Trail::Rocket | Trail::Smoke => {
                let offset = if kind == Trail::Rocket { 0.0 } else { 2.0 };
                particle.ramp = self.below(4) + offset;
                particle.color = RAMP_FIRE[particle.ramp as usize];
                particle.motion = Motion::Fire;
                particle.origin = at + self.spread(6, 3.0);
            }
            Trail::Blood | Trail::SlightBlood => {
                particle.color = 67 + (self.rand() & 3) as u8;
                particle.motion = Motion::Fall;
                particle.origin = at + self.spread(6, 3.0);
            }
            Trail::Tracer | Trail::Tracer2 => {
                let base = if kind == Trail::Tracer { 52 } else { 230 };
                particle.color = base + ((self.tracers & 4) << 1) as u8;
                particle.die = self.time + 0.5;
                self.tracers = self.tracers.wrapping_add(1);
                let side = if self.tracers % 2 == 1 { 30.0 } else { -30.0 };
                particle.velocity = Vec3::new(side * direction.y, -side * direction.x, 0.0);
            }
            Trail::Voor => {
                particle.color = 9 * 16 + 8 + (self.rand() & 3) as u8;
                particle.die = self.time + 0.3;
                particle.origin = at + self.spread(16, 8.0);
            }
        }
        self.add(particle);
    }

    /// Moves everything on to `time`, from `R_DrawParticles`.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a ramp position is small and never negative"
    )]
    pub fn update(&mut self, time: f32) {
        let seconds = (time - self.time).max(0.0);
        self.time = time;
        let fall = seconds * self.gravity * 0.05;
        let grow = 4.0 * seconds;
        self.live.retain_mut(|particle| {
            particle.origin = particle.origin + particle.velocity * seconds;
            match particle.motion {
                Motion::Still => {}
                Motion::Fire => {
                    particle.ramp += seconds * 5.0;
                    match RAMP_FIRE.get(particle.ramp as usize) {
                        Some(color) => particle.color = *color,
                        None => return false,
                    }
                    particle.velocity.z += fall;
                }
                Motion::Explode => {
                    particle.ramp += seconds * 10.0;
                    match RAMP_EXPLODE.get(particle.ramp as usize) {
                        Some(color) => particle.color = *color,
                        None => return false,
                    }
                    particle.velocity = particle.velocity + particle.velocity * grow;
                    particle.velocity.z -= fall;
                }
                Motion::Explode2 => {
                    particle.ramp += seconds * 15.0;
                    match RAMP_EXPLODE2.get(particle.ramp as usize) {
                        Some(color) => particle.color = *color,
                        None => return false,
                    }
                    particle.velocity = particle.velocity - particle.velocity * seconds;
                    particle.velocity.z -= fall;
                }
                Motion::Blob => {
                    particle.velocity = particle.velocity + particle.velocity * grow;
                    particle.velocity.z -= fall;
                }
                Motion::Blob2 => {
                    particle.velocity.x -= particle.velocity.x * grow;
                    particle.velocity.y -= particle.velocity.y * grow;
                    particle.velocity.z -= fall;
                }
                Motion::Fall => particle.velocity.z -= fall,
            }
            particle.die >= time
        });
    }
}
