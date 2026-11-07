//! Running effects on the CPU, and handing what's for the GPU over (proposal
//! 0114).

use std::collections::HashMap;
use std::sync::Arc;

use engine_core::entity::effect_id;
use engine_core::math::Vec3;
use engine_map::{EffectDef, EmitterShape, ParticleCollide, ParticleEmitter, ParticleKind};

/// The most particles the CPU looks after.
pub const MAX_EFFECT_PARTICLES: usize = 8192;

/// How much of the budget can go before what's far off starts to make less.
const BUDGET_EASY: f32 = 0.6;

/// What effects need of the world around them.
pub struct EffectWorld<'a> {
    /// Where an entity is now.
    pub place: &'a dyn Fn(u64) -> Option<Vec3>,
    /// What a line hits first, as a place and which way it faces.
    pub hit: &'a dyn Fn(Vec3, Vec3) -> Option<(Vec3, Vec3)>,
}

/// One instance's own settings, which multiply what its effect says.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectSettings {
    /// Its colour.
    pub color: [f32; 3],
    /// Its size.
    pub size: f32,
    /// How many it makes.
    pub strength: f32,
}

impl Default for EffectSettings {
    fn default() -> Self {
        Self {
            color: [1.0; 3],
            size: 1.0,
            strength: 1.0,
        }
    }
}

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
    /// How it's turned, in radians.
    pub rotation: f32,
    /// How far it fades into what's behind it.
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
    /// Their picture, plus one, or 0 for a soft dot.
    pub picture: u32,
    /// How far they fade into what's behind them.
    pub soft: f32,
}

/// An emitter compute runs, for one frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimEmitter {
    /// Which it is, which goes on being it from frame to frame.
    pub id: u64,
    /// Where it is.
    pub origin: Vec3,
    /// How many slots it wants.
    pub count: u32,
    /// How many start this step.
    pub spawn: u32,
    /// How long this step is.
    pub seconds: f32,
    /// The time.
    pub time: f32,
    /// How hard gravity pulls.
    pub gravity: f32,
    /// A number that makes this step differ from the last, and this emitter from others.
    pub seed: u32,
    /// What its instance says of itself.
    pub settings: EffectSettings,
    /// How much of it's left as it fades away, from 1 to 0.
    pub fade: f32,
    /// Which effect it's of, by its place in the list of those going.
    pub emitter: usize,
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
    /// How it's turned, in radians.
    rotation: f32,
    /// How fast it turns, in radians a second.
    spin: f32,
    /// The handle of what started it, or 0.
    owner: u32,
    /// The middle of what started it, when it did, for forces to go by.
    middle: Vec3,
    /// What that instance says.
    settings: EffectSettings,
    /// How long it has left, and how long it had, once it's fading.
    fading: Option<(f32, f32)>,
    /// Whether it has stuck to something.
    stuck: bool,
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
    /// What it's following, if it's anything: an entity by number, or a model by
    /// [`follow_model`].
    follows: Option<u64>,
    /// Where it is on a model that hasn't moved, if it's following one.
    rest: Option<Vec3>,
    /// A number that makes this one differ from the next.
    seed: u32,
    /// Whether it goes on until it's told to stop.
    lasting: bool,
    /// What a script calls it, or 0.
    handle: u32,
    /// What this one says of itself.
    settings: EffectSettings,
    /// Whether it's been told to stop, so that it makes no more.
    stopped: bool,
    /// How long what compute runs of it has left to fade, of how long it was given.
    fading: Option<(f32, f32)>,
    /// How many of its timeline have gone off.
    timeline_done: usize,
    /// How many each emitter compute runs starts this step.
    sim_spawn: Vec<u32>,
}

/// What an effect follows to go with `model`. A model isn't an entity, so it has numbers of its
/// own, which `place` gives how far it's moved for.
#[must_use]
pub const fn follow_model(model: u32) -> u64 {
    (1 << 63) | model as u64
}

/// The model that `follows` means, if it means one.
#[must_use]
pub const fn followed_model(follows: u64) -> Option<u32> {
    if follows >> 63 == 1 {
        #[expect(clippy::cast_possible_truncation, reason = "it was a u32 going in")]
        Some(follows as u32)
    } else {
        None
    }
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
    /// Where the view is, for what thins out with distance.
    eye: Vec3,
    /// How many particles can be going before what's far off makes less.
    budget: usize,
    /// How long it's been going, for what comes and goes.
    time: f32,
    /// How long the last step was.
    last_seconds: f32,
}

impl Default for EffectSystem {
    fn default() -> Self {
        Self {
            known: HashMap::new(),
            going: Vec::new(),
            live: Vec::new(),
            seed: 0x2545_f491,
            gravity: 800.0,
            eye: Vec3::ZERO,
            budget: MAX_EFFECT_PARTICLES,
            time: 0.0,
            last_seconds: 0.0,
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

    /// Changes what effects there are while they're going. What's going starts over as it now
    /// is, so what's timed can be seen. Particles there are go on as they were made.
    pub fn change_effects(&mut self, effects: &[Arc<EffectDef>]) {
        self.known = effects
            .iter()
            .map(|effect| (effect_id(&effect.name), Arc::clone(effect)))
            .collect();
        for effect in &mut self.going {
            let Some(def) = self.known.get(&effect_id(&effect.def.name)) else {
                continue;
            };
            let count = def.emitters.len();
            effect.def = Arc::clone(def);
            effect.age = 0.0;
            effect.timeline_done = 0;
            effect.owed = vec![0.0; count];
            effect.burst = vec![false; count];
            effect.sim_spawn = vec![0; count];
        }
    }

    /// Sets the world's gravity.
    pub const fn set_gravity(&mut self, gravity: f32) {
        self.gravity = gravity;
    }

    /// Says where the view is, which is what's near.
    pub const fn set_eye(&mut self, eye: Vec3) {
        self.eye = eye;
    }

    /// Sets how many particles can be going, from 1 to the most there can be.
    pub fn set_budget(&mut self, budget: usize) {
        self.budget = budget.clamp(1, MAX_EFFECT_PARTICLES);
    }

    /// How many effects are going.
    #[must_use]
    pub const fn going(&self) -> usize {
        self.going.len()
    }

    /// Stops the effect called `handle`: it makes no more, and what it's made fades over `seconds`.
    pub fn stop(&mut self, handle: u32, seconds: f32) {
        if handle == 0 {
            return;
        }
        for effect in self.going.iter_mut().filter(|e| e.handle == handle) {
            effect.stopped = true;
            effect.lasting = false;
            effect.sim_spawn.fill(0);
            let computed = effect.def.emitters.iter().any(|emitter| emitter.sim);
            effect.fading = (computed && seconds > 0.0).then_some((seconds, seconds));
        }
        if seconds <= 0.0 {
            self.live.retain(|particle| particle.owner != handle);
            return;
        }
        for particle in self.live.iter_mut().filter(|p| p.owner == handle) {
            particle.fading = Some((seconds, seconds));
        }
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

    /// Starts the effect with the hash `id`, going until the level does, on `model`, at
    /// `origin` when that hasn't moved.
    pub fn start_on_model(&mut self, id: u32, origin: Vec3, model: u32) {
        let before = self.going.len();
        self.start(id, origin, Some(follow_model(model)), true);
        if let Some(effect) = self.going.get_mut(before) {
            effect.rest = Some(origin);
        }
    }

    /// Starts the effect with the hash `id` at `origin`. It ends when it runs out,
    /// or goes on until the level does if `lasting`. With `follows`, it goes where
    /// that entity does.
    pub fn start(&mut self, id: u32, origin: Vec3, follows: Option<u64>, lasting: bool) {
        self.start_as(id, origin, follows, lasting, (0, EffectSettings::default()));
    }

    /// Starts the effect with the hash `id`, as [`Self::start`] does, with a handle to stop it by
    /// and settings of its own.
    pub fn start_as(
        &mut self,
        id: u32,
        origin: Vec3,
        follows: Option<u64>,
        lasting: bool,
        (handle, settings): (u32, EffectSettings),
    ) {
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
            rest: None,
            seed: self.seed,
            lasting,
            handle,
            settings,
            stopped: false,
            fading: None,
            timeline_done: 0,
            sim_spawn: vec![0; count],
        });
    }

    /// Starts one particle of `emitter`, for an instance with this `owner` and these settings.
    fn spawn(
        &mut self,
        effect: &Arc<EffectDef>,
        number: usize,
        origin: Vec3,
        (owner, settings): (u32, EffectSettings),
    ) {
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
        let (rotation, spin) = (emitter.rotation, emitter.spin);
        let outward = emitter.outward;
        let offset = self.shape_offset(shape);
        let start = origin + offset;
        let heading = if outward && offset.length() > 0.001 {
            offset.normalized()
        } else {
            self.heading(direction, spread)
        };
        let speed = self.between(speed);
        let life = self.between(life);
        let rotation = self.between(rotation).to_radians();
        let spin = self.between(spin).to_radians();
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
            rotation,
            spin,
            owner,
            middle: origin,
            settings,
            fading: None,
            stuck: false,
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
            EmitterShape::Ring(radius) => {
                let angle = self.random() * std::f32::consts::TAU;
                Vec3::new(angle.cos() * radius, angle.sin() * radius, 0.0)
            }
            EmitterShape::Disc(radius) => {
                let angle = self.random() * std::f32::consts::TAU;
                let out = radius * self.random().sqrt();
                Vec3::new(angle.cos() * out, angle.sin() * out, 0.0)
            }
            EmitterShape::Cone(radius, height) => {
                let angle = self.random() * std::f32::consts::TAU;
                // Evenly by volume, which there's more of towards the top.
                let up = self.random().cbrt();
                let out = radius * up * self.random().sqrt();
                Vec3::new(angle.cos() * out, angle.sin() * out, height * up)
            }
            EmitterShape::Line(half) => half * (self.random() * 2.0 - 1.0),
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
    pub fn update(&mut self, seconds: f32, world: &EffectWorld<'_>) {
        self.time += seconds;
        self.last_seconds = seconds;
        self.seed = self.seed.wrapping_add(1);
        let mut going = std::mem::take(&mut self.going);
        for effect in &mut going {
            if let Some(origin) = effect.follows.and_then(world.place) {
                effect.origin = effect.rest.map_or(origin, |rest| rest + origin);
            }
            effect.age += seconds;
            let def = Arc::clone(&effect.def);
            if effect.stopped {
                if let Some((left, _)) = &mut effect.fading {
                    *left -= seconds;
                }
                continue;
            }
            for (number, emitter) in def.emitters.iter().enumerate() {
                self.run_emitter(effect, &def, number, emitter, seconds);
            }
            self.run_timeline(effect, &def);
        }
        going.retain(|effect| {
            if effect.stopped {
                effect.fading.is_some_and(|(left, _)| left > 0.0)
            } else {
                effect.lasting || !effect_is_over(effect)
            }
        });
        self.going = going;
        let gravity = self.gravity;
        let mut ended = Vec::new();
        let time_seed = self.seed;
        let time = self.time;
        for (number, particle) in (0_u32..).zip(self.live.iter_mut()) {
            let Some(emitter) = particle.effect.emitters.get(particle.emitter) else {
                continue;
            };
            particle.age += seconds;
            if let Some((left, _)) = &mut particle.fading {
                *left -= seconds;
            }
            if particle.stuck {
                if particle.age >= particle.life
                    && let Some(next) = &emitter.on_death
                {
                    let from = (particle.owner, particle.settings);
                    ended.push((
                        Arc::clone(&particle.effect),
                        next.clone(),
                        particle.origin,
                        from,
                    ));
                }
                continue;
            }
            for force in &emitter.forces {
                let push = force.push(particle.origin, particle.middle, time);
                particle.velocity = particle.velocity + push * seconds;
            }
            particle.velocity.z -= gravity * emitter.gravity * seconds;
            particle.velocity = particle.velocity * (1.0 - emitter.drag * seconds).max(0.0);
            if emitter.turbulence > 0.0 {
                let noise = |shift: u32| {
                    hash_unit(time_seed.wrapping_add(shift).wrapping_add(number)) - 0.5
                };
                let push = Vec3::new(noise(1), noise(2), noise(3));
                particle.velocity = particle.velocity + push * (emitter.turbulence * seconds * 8.0);
            }
            let was = particle.origin;
            particle.origin = particle.origin + particle.velocity * seconds;
            particle.rotation += particle.spin * seconds;
            if emitter.collide != ParticleCollide::None
                && let Some((at, normal)) = (world.hit)(was, particle.origin)
            {
                // A touch off the surface, so that it isn't in it.
                particle.origin = at + normal * 0.25;
                particle.facing = normal;
                match emitter.collide {
                    ParticleCollide::Bounce => {
                        let into = particle.velocity.dot(normal);
                        particle.velocity =
                            (particle.velocity - normal * (into * 2.0)) * emitter.bounce;
                    }
                    ParticleCollide::Stick => {
                        particle.velocity = Vec3::ZERO;
                        particle.stuck = true;
                    }
                    ParticleCollide::Die => particle.age = particle.life,
                    ParticleCollide::None => {}
                }
                if let Some(next) = &emitter.on_hit {
                    let from = (particle.owner, particle.settings);
                    ended.push((
                        Arc::clone(&particle.effect),
                        next.clone(),
                        particle.origin,
                        from,
                    ));
                }
            }
            if emitter.kind == ParticleKind::Ribbon {
                particle.trail.push(particle.origin);
                if particle.trail.len() > RIBBON_POINTS {
                    particle.trail.remove(0);
                }
            }
            if particle.age >= particle.life
                && let Some(next) = &emitter.on_death
            {
                let from = (particle.owner, particle.settings);
                ended.push((
                    Arc::clone(&particle.effect),
                    next.clone(),
                    particle.origin,
                    from,
                ));
            }
        }
        self.live.retain(|particle| {
            particle.age < particle.life && particle.fading.is_none_or(|(left, _)| left > 0.0)
        });
        for (effect, name, origin, from) in ended {
            self.burst_of(&effect, &name, origin, from);
        }
    }

    /// Starts `name`'s burst, or one of it if it has none.
    fn burst_of(
        &mut self,
        effect: &Arc<EffectDef>,
        name: &str,
        origin: Vec3,
        from: (u32, EffectSettings),
    ) {
        let found = effect.emitters.iter().position(|e| e.name == name);
        if let Some(number) = found {
            let burst = effect.emitters[number].burst.max(1);
            for _ in 0..burst {
                self.spawn(effect, number, origin, from);
            }
        }
    }

    /// Sets off what `effect`'s timeline has come to.
    fn run_timeline(&mut self, effect: &mut Going, def: &Arc<EffectDef>) {
        while let Some((at, name)) = def.timeline.get(effect.timeline_done) {
            if *at > effect.age {
                break;
            }
            effect.timeline_done += 1;
            self.burst_of(def, name, effect.origin, (effect.handle, effect.settings));
        }
    }

    /// How much of what `emitter` would make it makes, at `origin`: it thins out towards `far`,
    /// and with the budget going, what's far off gives way first.
    fn share_made(&self, emitter: &ParticleEmitter, origin: Vec3) -> f32 {
        let away = (origin - self.eye).length();
        let mut share = 1.0;
        if emitter.far > 0.0 {
            share *= (1.0 - (away - emitter.far * 0.5) / (emitter.far * 0.5)).clamp(0.0, 1.0);
        }
        #[expect(clippy::cast_precision_loss, reason = "counts are small")]
        let used = self.live.len() as f32 / self.budget as f32;
        if used > BUDGET_EASY {
            let tight = ((used - BUDGET_EASY) / (1.0 - BUDGET_EASY)).clamp(0.0, 1.0);
            let near = (1.0 - away / 2048.0).clamp(0.0, 1.0);
            share *= 1.0 - tight * (1.0 - near * 0.8);
        }
        share
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
        effect.sim_spawn[number] = 0;
        if emitter.gpu || started_by_another(def, emitter) || effect.age < emitter.delay {
            return;
        }
        let running = effect.age - emitter.delay;
        if emitter.duration > 0.0 && running > emitter.duration {
            return;
        }
        let share = self.share_made(emitter, effect.origin) * effect.settings.strength;
        if emitter.sim {
            run_sim(effect, number, emitter, seconds, share);
            return;
        }
        let from = (effect.handle, effect.settings);
        if !effect.burst[number] {
            effect.burst[number] = true;
            #[expect(clippy::cast_precision_loss, reason = "counts are small")]
            let many = emitter.burst as f32 * share;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a count of particles"
            )]
            for _ in 0..many.round().max(0.0) as u32 {
                self.spawn(def, number, effect.origin, from);
            }
        }
        effect.owed[number] += emitter.rate * seconds * share;
        while effect.owed[number] >= 1.0 {
            effect.owed[number] -= 1.0;
            self.spawn(def, number, effect.origin, from);
        }
    }

    /// What's to be drawn of the particles, and the lights among them. `light` says how bright it
    /// is at a place, for those that take light from the level.
    #[must_use]
    pub fn draws(&self, light: &dyn Fn(Vec3) -> [f32; 3]) -> (Vec<EffectDraw>, Vec<EffectLight>) {
        let mut draws = Vec::with_capacity(self.live.len());
        let mut lights = Vec::new();
        for particle in &self.live {
            let Some(emitter) = particle.effect.emitters.get(particle.emitter) else {
                continue;
            };
            let at = particle.age / particle.life;
            let mut color = emitter.color.at(at);
            for (part, own) in color.iter_mut().zip(particle.settings.color) {
                *part *= own;
            }
            if let Some((left, whole)) = particle.fading {
                color[3] *= (left / whole).clamp(0.0, 1.0);
            }
            if emitter.lit {
                let here = light(particle.origin);
                for (part, light) in color.iter_mut().zip(here) {
                    *part *= light;
                }
            }
            let size = emitter.size.at(at) * particle.settings.size;
            let adds = emitter.blend == engine_map::ParticleBlend::Add;
            let look = EffectDraw {
                origin: particle.origin,
                end: particle.origin,
                color,
                size,
                kind: emitter.kind,
                adds,
                rotation: particle.rotation,
                soft: emitter.soft,
                picture: emitter.picture,
                frame: frame_of(emitter, particle.age, at),
                sheet: emitter.flipbook,
                life: at,
                shader: emitter.shader_number,
            };
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
                            ..look
                        });
                    }
                }
                ParticleKind::Beam => draws.push(EffectDraw {
                    end: particle.origin + emitter.direction.normalized() * emitter.reach,
                    ..look
                }),
                ParticleKind::Decal => draws.push(EffectDraw {
                    end: particle.facing,
                    ..look
                }),
                ParticleKind::Sprite if emitter.stretch > 0.0 => {
                    // Drawn out along how it's moving, as a line through where it is.
                    let along = particle.velocity * (emitter.stretch * 0.5);
                    draws.push(EffectDraw {
                        origin: particle.origin - along,
                        end: particle.origin + along,
                        kind: ParticleKind::Ribbon,
                        ..look
                    });
                }
                ParticleKind::Sprite => draws.push(look),
            }
        }
        (draws, lights)
    }

    /// What compute runs this frame, as each emitter with where it is and what's new, and the
    /// effect and emitter it comes from.
    #[must_use]
    pub fn sims(&self) -> Vec<(SimEmitter, &ParticleEmitter)> {
        let mut out = Vec::new();
        for (place, effect) in self.going.iter().enumerate() {
            let fade = effect
                .fading
                .map_or(1.0, |(left, whole)| (left / whole).clamp(0.0, 1.0));
            // What another starts begins where that one was, so it's a burst of the usual kind.
            for (number, emitter) in effect
                .def
                .emitters
                .iter()
                .enumerate()
                .filter(|(_, e)| e.sim && !started_by_another(&effect.def, e))
            {
                let own = effect.seed.wrapping_mul(0x9e37_79b1);
                let own = own.wrapping_add(u32::try_from(number).unwrap_or(0));
                let life = emitter.life[1].max(0.01);
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "counts are small"
                )]
                let count = ((emitter.rate * life + emitter.burst as f32)
                    * effect.settings.strength)
                    .ceil()
                    .max(0.0) as u32
                    + 16;
                out.push((
                    SimEmitter {
                        id: (u64::from(effect.seed) << 8) | number as u64,
                        origin: effect.origin,
                        count,
                        spawn: effect.sim_spawn.get(number).copied().unwrap_or(0),
                        seconds: self.last_seconds,
                        time: self.time,
                        gravity: self.gravity * emitter.gravity,
                        seed: self.seed.wrapping_add(own),
                        settings: effect.settings,
                        fade,
                        emitter: place,
                    },
                    emitter,
                ));
            }
        }
        out
    }

    /// What the GPU works out this frame.
    #[must_use]
    pub fn gpu_emitters(&self) -> Vec<GpuEmitter> {
        let mut out = Vec::new();
        for effect in self.going.iter().filter(|effect| !effect.stopped) {
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
                    EmitterShape::Ball(radius)
                    | EmitterShape::Ring(radius)
                    | EmitterShape::Disc(radius)
                    | EmitterShape::Cone(radius, _) => radius,
                    EmitterShape::Box(half) | EmitterShape::Line(half) => {
                        half.x.max(half.y).max(half.z)
                    }
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
                    picture: emitter.picture,
                    soft: emitter.soft,
                });
            }
        }
        out
    }
}

/// Which frame of its sheet a particle of `emitter` is on, `age` seconds in, and `share` of the way
/// through its life.
fn frame_of(emitter: &ParticleEmitter, age: f32, share: f32) -> f32 {
    let [across, down] = emitter.flipbook;
    #[expect(clippy::cast_precision_loss, reason = "a sheet has few frames")]
    let frames = (across * down).max(1) as f32;
    if emitter.flip_rate > 0.0 {
        (age * emitter.flip_rate) % frames
    } else {
        // Over its life, ending on its last frame, with nothing past it to mix with.
        (share * (frames - 1.0)).clamp(0.0, frames - 1.0)
    }
}

/// Works out how many of `emitter`, which compute runs, start in `seconds`.
fn run_sim(effect: &mut Going, number: usize, emitter: &ParticleEmitter, seconds: f32, share: f32) {
    let mut spawn = 0;
    if !effect.burst[number] {
        effect.burst[number] = true;
        #[expect(clippy::cast_precision_loss, reason = "a count of particles")]
        let many = emitter.burst as f32 * share;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a count of particles"
        )]
        {
            spawn += many.round().max(0.0) as u32;
        }
    }
    effect.owed[number] += emitter.rate * seconds * share;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a count of particles"
    )]
    let whole = effect.owed[number].floor().max(0.0) as u32;
    #[expect(clippy::cast_precision_loss, reason = "a count of particles")]
    let taken = whole as f32;
    effect.owed[number] -= taken;
    effect.sim_spawn[number] = spawn + whole;
}

/// Whether another emitter starts `emitter`, so it doesn't run by itself.
fn started_by_another(def: &EffectDef, emitter: &ParticleEmitter) -> bool {
    let name = Some(emitter.name.as_str());
    def.timeline.iter().any(|(_, start)| *start == emitter.name)
        || def
            .emitters
            .iter()
            .any(|other| other.on_death.as_deref() == name || other.on_hit.as_deref() == name)
}

/// Whether every emitter of `effect` has finished.
fn effect_is_over(effect: &Going) -> bool {
    let timeline_done = effect.timeline_done >= effect.def.timeline.len();
    timeline_done
        && effect.def.emitters.iter().all(|emitter| {
            let over = emitter.delay + emitter.duration;
            let endless = emitter.duration <= 0.0 && emitter.rate > 0.0;
            !endless && effect.age > over + emitter.life[1]
        })
}
