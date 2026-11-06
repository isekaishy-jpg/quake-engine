//! Running effects on the CPU, and handing what's for the GPU over (proposal
//! 0114).

use std::collections::HashMap;
use std::sync::Arc;

use engine_core::entity::effect_id;
use engine_core::math::Vec3;
use engine_map::{EffectDef, EmitterShape, ParticleEmitter, ParticleKind};

/// The most particles the CPU looks after.
pub const MAX_EFFECT_PARTICLES: usize = 8192;

/// The most effects going at once.
const MAX_EFFECTS: usize = 256;

/// How many places a ribbon remembers.
const RIBBON_POINTS: usize = 12;

/// What's drawn of a particle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectDraw {
    /// Where it is.
    pub origin: Vec3,
    /// The far end of what's a line, or which way a flat mark faces.
    pub end: Vec3,
    /// Its colour and alpha.
    pub color: [f32; 4],
    /// Its size.
    pub size: f32,
    /// What it is.
    pub kind: ParticleKind,
    /// Whether it adds to what's behind it.
    pub adds: bool,
}

/// A light from an effect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectLight {
    /// Where it is.
    pub origin: Vec3,
    /// How far it reaches.
    pub reach: f32,
    /// Its colour.
    pub color: [f32; 3],
}

/// An emitter the GPU works out, for one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuEmitter {
    /// Where it is.
    pub origin: Vec3,
    /// Which way they head.
    pub direction: Vec3,
    /// How far from that they can go, as a cosine.
    pub spread: f32,
    /// How long since it began.
    pub age: f32,
    /// How many there are.
    pub count: u32,
    /// How long each lives.
    pub life: f32,
    /// How fast they start.
    pub speed: f32,
    /// How hard gravity pulls.
    pub gravity: f32,
    /// How hard they're pushed about.
    pub turbulence: f32,
    /// Their size at the start and end.
    pub size: [f32; 2],
    /// Their colour at the start.
    pub color: [f32; 4],
    /// Their colour at the end.
    pub end_color: [f32; 4],
    /// Whether they add to what's behind them.
    pub adds: bool,
    /// How far they start from it.
    pub radius: f32,
}

/// One particle.
#[derive(Clone, Debug)]
struct Live {
    /// Which effect it's from.
    effect: Arc<EffectDef>,
    /// Which of its emitters.
    emitter: usize,
    /// Where it is.
    origin: Vec3,
    /// How it's moving.
    velocity: Vec3,
    /// How long it's been going.
    age: f32,
    /// How long it has.
    life: f32,
    /// Where it's been, for a ribbon.
    trail: Vec<Vec3>,
    /// Whether it's a flat mark that has landed, facing this way.
    facing: Vec3,
}

/// An effect that's going.
#[derive(Clone, Debug)]
struct Going {
    /// What it is.
    def: Arc<EffectDef>,
    /// Where it is.
    origin: Vec3,
    /// How long it's been going.
    age: f32,
    /// What each emitter hasn't yet started, as a part of one.
    owed: Vec<f32>,
    /// Whether each emitter's burst has gone.
    burst: Vec<bool>,
    /// What it's following, if it's anything: an entity by number.
    follows: Option<u64>,
    /// A number that makes this one differ from the next.
    seed: u32,
    /// Whether it goes on until it's told to stop.
    lasting: bool,
}

/// Every effect going.
#[derive(Clone, Debug)]
pub struct EffectSystem {
    /// What can be started, by the hash of a name.
    known: HashMap<u32, Arc<EffectDef>>,
    /// What's going.
    going: Vec<Going>,
    /// The particles.
    live: Vec<Live>,
    /// For what's random.
    seed: u32,
    /// The world's gravity.
    gravity: f32,
}

impl Default for EffectSystem {
    fn default() -> Self {
        Self {
            known: HashMap::new(),
            going: Vec::new(),
            live: Vec::new(),
            seed: 0x2545_f491,
            gravity: 800.0,
        }
    }
}

/// A number from `seed`, from 0 to 1.
fn hash_unit(seed: u32) -> f32 {
    let mut value = seed.wrapping_mul(0x9e37_79b1);
    value ^= value >> 15;
    value = value.wrapping_mul(0x85eb_ca77);
    value ^= value >> 13;
    #[expect(clippy::cast_precision_loss, reason = "a share has no need of more")]
    let share = (value & 0x00ff_ffff) as f32 / 16_777_216.0;
    share
}

impl EffectSystem {
    /// Sets what can be started, for a new level, and stops all that's going.
    pub fn set_effects(&mut self, effects: &[Arc<EffectDef>]) {
        self.known = effects
            .iter()
            .map(|effect| (effect_id(&effect.name), Arc::clone(effect)))
            .collect();
        self.going.clear();
        self.live.clear();
    }

    /// Sets the world's gravity.
    pub const fn set_gravity(&mut self, gravity: f32) {
        self.gravity = gravity;
    }

    /// How many particles are alive.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.live.len()
    }

    /// A number from 0 to 1.
    fn random(&mut self) -> f32 {
        self.seed = self.seed.wrapping_add(0x6d2b_79f5);
        hash_unit(self.seed)
    }

    /// A number between the two ends of `range`.
    fn between(&mut self, range: [f32; 2]) -> f32 {
        range[0] + (range[1] - range[0]) * self.random()
    }

    /// Starts the effect with the hash `id` at `origin`. It ends when it runs out,
    /// or goes on until the level does if `lasting`. With `follows`, it goes where
    /// that entity does.
    pub fn start(&mut self, id: u32, origin: Vec3, follows: Option<u64>, lasting: bool) {
        let Some(def) = self.known.get(&id).cloned() else {
            return;
        };
        if self.going.len() >= MAX_EFFECTS {
            return;
        }
        let count = def.emitters.len();
        self.seed = self.seed.wrapping_add(1);
        self.going.push(Going {
            def,
            origin,
            age: 0.0,
            owed: vec![0.0; count],
            burst: vec![false; count],
            follows,
            seed: self.seed,
            lasting,
        });
    }

    /// Starts one particle of `emitter`.
    fn spawn(&mut self, effect: &Arc<EffectDef>, number: usize, origin: Vec3) {
        if self.live.len() >= MAX_EFFECT_PARTICLES {
            return;
        }
        let Some(emitter) = effect.emitters.get(number) else {
            return;
        };
        let (shape, life, speed, direction, spread, kind) = (
            emitter.shape,
            emitter.life,
            emitter.speed,
            emitter.direction,
            emitter.spread,
            emitter.kind,
        );
        let start = origin + self.shape_offset(shape);
        let heading = self.heading(direction, spread);
        let speed = self.between(speed);
        let life = self.between(life);
        self.live.push(Live {
            effect: Arc::clone(effect),
            emitter: number,
            origin: start,
            velocity: heading * speed,
            age: 0.0,
            life: life.max(0.01),
            trail: if kind == ParticleKind::Ribbon {
                vec![start]
            } else {
                Vec::new()
            },
            facing: Vec3::new(0.0, 0.0, 1.0),
        });
    }

    /// How far from its emitter something in `shape` starts.
    fn shape_offset(&mut self, shape: EmitterShape) -> Vec3 {
        match shape {
            EmitterShape::Point => Vec3::ZERO,
            EmitterShape::Ball(radius) => {
                let direction = self.heading(Vec3::new(0.0, 0.0, 1.0), 180.0);
                direction * (radius * self.random().cbrt())
            }
            EmitterShape::Box(half) => Vec3::new(
                half.x * (self.random() * 2.0 - 1.0),
                half.y * (self.random() * 2.0 - 1.0),
                half.z * (self.random() * 2.0 - 1.0),
            ),
        }
    }

    /// A direction within `spread` degrees of `direction`.
    fn heading(&mut self, direction: Vec3, spread: f32) -> Vec3 {
        let cosine = spread.to_radians().cos();
        let z = 1.0 - self.random() * (1.0 - cosine);
        let angle = self.random() * std::f32::consts::TAU;
        let ring = (1.0 - z * z).max(0.0).sqrt();
        let axis = direction.normalized();
        let helper = if axis.z.abs() < 0.9 {
            Vec3::new(0.0, 0.0, 1.0)
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        };
        let side = axis.cross(helper).normalized();
        let up = axis.cross(side);
        side * (ring * angle.cos()) + up * (ring * angle.sin()) + axis * z
    }

    /// Moves everything on by `seconds`. `place` says where an entity is now, and
    /// `ground` how far down something is from a point, for a mark to land on.
    pub fn update(&mut self, seconds: f32, place: &dyn Fn(u64) -> Option<Vec3>) {
        let mut going = std::mem::take(&mut self.going);
        for effect in &mut going {
            if let Some(origin) = effect.follows.and_then(place) {
                effect.origin = origin;
            }
            effect.age += seconds;
            let def = Arc::clone(&effect.def);
            for (number, emitter) in def.emitters.iter().enumerate() {
                self.run_emitter(effect, &def, number, emitter, seconds);
            }
        }
        going.retain(|effect| effect.lasting || !effect_is_over(effect));
        self.going = going;
        let gravity = self.gravity;
        let mut ended = Vec::new();
        let time_seed = self.seed;
        for (number, particle) in (0_u32..).zip(self.live.iter_mut()) {
            let Some(emitter) = particle.effect.emitters.get(particle.emitter) else {
                continue;
            };
            particle.age += seconds;
            particle.velocity.z -= gravity * emitter.gravity * seconds;
            particle.velocity = particle.velocity * (1.0 - emitter.drag * seconds).max(0.0);
            if emitter.turbulence > 0.0 {
                let noise = |shift: u32| {
                    hash_unit(time_seed.wrapping_add(shift).wrapping_add(number)) - 0.5
                };
                let push = Vec3::new(noise(1), noise(2), noise(3));
                particle.velocity = particle.velocity + push * (emitter.turbulence * seconds * 8.0);
            }
            particle.origin = particle.origin + particle.velocity * seconds;
            if emitter.kind == ParticleKind::Ribbon {
                particle.trail.push(particle.origin);
                if particle.trail.len() > RIBBON_POINTS {
                    particle.trail.remove(0);
                }
            }
            if particle.age >= particle.life
                && let Some(next) = &emitter.on_death
            {
                ended.push((Arc::clone(&particle.effect), next.clone(), particle.origin));
            }
        }
        self.live.retain(|particle| particle.age < particle.life);
        for (effect, name, origin) in ended {
            let found = effect.emitters.iter().position(|e| e.name == name);
            if let Some(number) = found {
                let burst = effect.emitters[number].burst.max(1);
                for _ in 0..burst {
                    self.spawn(&effect, number, origin);
                }
            }
        }
    }

    /// Runs `emitter` of `effect` for `seconds`.
    fn run_emitter(
        &mut self,
        effect: &mut Going,
        def: &Arc<EffectDef>,
        number: usize,
        emitter: &ParticleEmitter,
        seconds: f32,
    ) {
        if emitter.gpu || started_by_another(def, emitter) || effect.age < emitter.delay {
            return;
        }
        let running = effect.age - emitter.delay;
        if emitter.duration > 0.0 && running > emitter.duration {
            return;
        }
        if !effect.burst[number] {
            effect.burst[number] = true;
            for _ in 0..emitter.burst {
                self.spawn(def, number, effect.origin);
            }
        }
        effect.owed[number] += emitter.rate * seconds;
        while effect.owed[number] >= 1.0 {
            effect.owed[number] -= 1.0;
            self.spawn(def, number, effect.origin);
        }
    }

    /// What's to be drawn of the particles, and the lights among them.
    #[must_use]
    pub fn draws(&self) -> (Vec<EffectDraw>, Vec<EffectLight>) {
        let mut draws = Vec::with_capacity(self.live.len());
        let mut lights = Vec::new();
        for particle in &self.live {
            let Some(emitter) = particle.effect.emitters.get(particle.emitter) else {
                continue;
            };
            let at = particle.age / particle.life;
            let color = emitter.color.at(at);
            let size = emitter.size.at(at);
            let adds = emitter.blend == engine_map::ParticleBlend::Add;
            match emitter.kind {
                ParticleKind::Light => lights.push(EffectLight {
                    origin: particle.origin,
                    reach: emitter.reach * color[3],
                    color: [color[0], color[1], color[2]],
                }),
                ParticleKind::Ribbon => {
                    for pair in particle.trail.windows(2) {
                        draws.push(EffectDraw {
                            origin: pair[0],
                            end: pair[1],
                            color,
                            size,
                            kind: ParticleKind::Ribbon,
                            adds,
                        });
                    }
                }
                ParticleKind::Beam => draws.push(EffectDraw {
                    origin: particle.origin,
                    end: particle.origin + emitter.direction.normalized() * emitter.reach,
                    color,
                    size,
                    kind: ParticleKind::Beam,
                    adds,
                }),
                ParticleKind::Decal => draws.push(EffectDraw {
                    origin: particle.origin,
                    end: particle.facing,
                    color,
                    size,
                    kind: ParticleKind::Decal,
                    adds,
                }),
                ParticleKind::Sprite => draws.push(EffectDraw {
                    origin: particle.origin,
                    end: particle.origin,
                    color,
                    size,
                    kind: ParticleKind::Sprite,
                    adds,
                }),
            }
        }
        (draws, lights)
    }

    /// What the GPU works out this frame.
    #[must_use]
    pub fn gpu_emitters(&self) -> Vec<GpuEmitter> {
        let mut out = Vec::new();
        for effect in &self.going {
            for emitter in effect.def.emitters.iter().filter(|e| e.gpu) {
                let running = effect.age - emitter.delay;
                if running < 0.0 {
                    continue;
                }
                let life = emitter.life[1].max(0.01);
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "counts are small"
                )]
                let count = (emitter.rate * life).ceil().max(0.0) as u32 + emitter.burst;
                let radius = match emitter.shape {
                    EmitterShape::Ball(radius) => radius,
                    EmitterShape::Box(half) => half.x.max(half.y).max(half.z),
                    EmitterShape::Point => 0.0,
                };
                out.push(GpuEmitter {
                    origin: effect.origin,
                    direction: emitter.direction.normalized(),
                    spread: emitter.spread.to_radians().cos(),
                    age: running + hash_unit(effect.seed) * life,
                    count,
                    life,
                    speed: emitter.speed[1],
                    gravity: self.gravity * emitter.gravity,
                    turbulence: emitter.turbulence,
                    size: [emitter.size.at(0.0), emitter.size.at(1.0)],
                    color: emitter.color.at(0.0),
                    end_color: emitter.color.at(1.0),
                    adds: emitter.blend == engine_map::ParticleBlend::Add,
                    radius,
                });
            }
        }
        out
    }
}

/// Whether another emitter starts `emitter`, so it doesn't run by itself.
fn started_by_another(def: &EffectDef, emitter: &ParticleEmitter) -> bool {
    def.emitters
        .iter()
        .any(|other| other.on_death.as_deref() == Some(emitter.name.as_str()))
}

/// Whether every emitter of `effect` has finished.
fn effect_is_over(effect: &Going) -> bool {
    effect.def.emitters.iter().all(|emitter| {
        let over = emitter.delay + emitter.duration;
        let endless = emitter.duration <= 0.0 && emitter.rate > 0.0;
        !endless && effect.age > over + emitter.life[1]
    })
}
