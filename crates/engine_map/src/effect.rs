//! Effects: particles an author describes (proposal 0114).
//!
//! An effect is a stack of modules for each of its emitters, which say how
//! particles start, what moves them, how they change and what they are. Emitters
//! can start each other, which makes a graph of them.

use engine_core::math::Vec3;

/// What a particle is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ParticleKind {
    /// A soft round picture facing the eye.
    #[default]
    Sprite,
    /// A strip that follows where it's been, as a trail does.
    Ribbon,
    /// A line from an emitter to where it's aimed.
    Beam,
    /// A light.
    Light,
    /// A flat mark on what's below it.
    Decal,
}

/// How a particle's colour joins what's behind it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ParticleBlend {
    /// It covers what's behind, by its alpha, as smoke does.
    #[default]
    Over,
    /// It adds to what's behind, as fire does.
    Add,
}

/// Where particles start, round an emitter.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum EmitterShape {
    /// At one place.
    #[default]
    Point,
    /// Anywhere in a ball of this radius.
    Ball(f32),
    /// Round a ring this wide, flat on the ground.
    Ring(f32),
    /// Over a flat circle this wide.
    Disc(f32),
    /// Through a cone this wide at its top, and this tall, with its point at the middle.
    Cone(f32, f32),
    /// Along a line, this far each way from its middle.
    Line(Vec3),
    /// Anywhere in a box of these half sizes.
    Box(Vec3),
}

/// A number that changes over a particle's life, from 0 to 1, as stops.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Curve(pub Vec<(f32, f32)>);

impl Curve {
    /// The value at `at`, between stops.
    #[must_use]
    pub fn at(&self, at: f32) -> f32 {
        let (Some(first), Some(last)) = (self.0.first(), self.0.last()) else {
            return 0.0;
        };
        if at <= first.0 {
            return first.1;
        }
        for pair in self.0.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            if at <= to.0 {
                let share = (at - from.0) / (to.0 - from.0).max(1e-6);
                return from.1 + (to.1 - from.1) * share;
            }
        }
        last.1
    }
}

/// A colour that changes over a particle's life, as stops.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ColorCurve(pub Vec<(f32, [f32; 4])>);

impl ColorCurve {
    /// The colour at `at`, between stops.
    #[must_use]
    pub fn at(&self, at: f32) -> [f32; 4] {
        let (Some(first), Some(last)) = (self.0.first(), self.0.last()) else {
            return [1.0; 4];
        };
        if at <= first.0 {
            return first.1;
        }
        for pair in self.0.windows(2) {
            let (from, to) = (pair[0], pair[1]);
            if at <= to.0 {
                let share = (at - from.0) / (to.0 - from.0).max(1e-6);
                return std::array::from_fn(|part| {
                    from.1[part] + (to.1[part] - from.1[part]) * share
                });
            }
        }
        last.1
    }
}

/// One emitter of an effect.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleEmitter {
    /// What it's called, for others to start it.
    pub name: String,
    /// What its particles are.
    pub kind: ParticleKind,
    /// How they blend.
    pub blend: ParticleBlend,
    /// How many start each second.
    pub rate: f32,
    /// How many start at once when it does.
    pub burst: u32,
    /// How many seconds it goes on for, or 0 for as long as its effect is there.
    pub duration: f32,
    /// How long after its effect starts it waits.
    pub delay: f32,
    /// Where they start.
    pub shape: EmitterShape,
    /// The shortest and longest they live.
    pub life: [f32; 2],
    /// The slowest and fastest they start.
    pub speed: [f32; 2],
    /// Which way they head.
    pub direction: Vec3,
    /// How far from that they can go, in degrees.
    pub spread: f32,
    /// How much of the world's gravity pulls them.
    pub gravity: f32,
    /// How much they slow, as a share each second.
    pub drag: f32,
    /// How hard they're pushed about.
    pub turbulence: f32,
    /// Their size in world units.
    pub size: Curve,
    /// Their colour, in light that can go past 1 for what glows.
    pub color: ColorCurve,
    /// How far a light reaches, or a beam goes.
    pub reach: f32,
    /// Whether the GPU works out where they are, which is for great numbers of
    /// them, and leaves out what depends on the one before.
    pub gpu: bool,
    /// The emitter each one starts when it goes.
    pub on_death: Option<String>,
    /// The picture each one shows, if it's not a soft dot.
    pub image: Option<String>,
    /// Which of [`crate::Map::effect_pictures`] that is, plus one, once a level's read it.
    pub picture: u32,
    /// The columns and rows of frames in its picture.
    pub flipbook: [u32; 2],
    /// How many frames a second, or 0 to go through them all over its life.
    pub flip_rate: f32,
    /// A shader of its own, by name.
    pub shader: Option<String>,
    /// Which of [`crate::Map::effect_shaders`] that is, plus one, once a level's read it.
    pub shader_number: u32,
    /// How far it starts turned, in degrees, from and to.
    pub rotation: [f32; 2],
    /// How fast it turns, in degrees a second, from and to.
    pub spin: [f32; 2],
    /// How far it's drawn out along how it's moving: its size, and this long for each unit a
    /// second.
    pub stretch: f32,
    /// How far it fades into what's behind it, in world units.
    pub soft: f32,
    /// Whether it takes light from the level.
    pub lit: bool,
    /// What happens when it hits something.
    pub collide: ParticleCollide,
    /// How much of its speed it keeps when it bounces.
    pub bounce: f32,
    /// The emitter each one starts when it hits something.
    pub on_hit: Option<String>,
    /// What pushes it about.
    pub forces: Vec<ParticleForce>,
    /// Whether it heads out from the middle of its shape, and not along `direction`.
    pub outward: bool,
    /// How far from the view it thins out to nothing, or 0 for no limit.
    pub far: f32,
}

/// What a particle does when it hits something.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ParticleCollide {
    /// Nothing: it goes through.
    #[default]
    None,
    /// It comes off.
    Bounce,
    /// It stays where it hit.
    Stick,
    /// It goes.
    Die,
}

/// Something that pushes particles about, in units a second, each second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ParticleForce {
    /// Towards a place, from the effect's middle, within a reach. Below 0 pushes away.
    Attract {
        /// Where, from the effect's middle.
        at: Vec3,
        /// How hard.
        strength: f32,
        /// How far it reaches, or 0 for as far as there is.
        reach: f32,
    },
    /// Round a line through a place, from the effect's middle.
    Vortex {
        /// Where, from the effect's middle.
        at: Vec3,
        /// Which way the line goes.
        axis: Vec3,
        /// How hard.
        strength: f32,
        /// How hard it draws them in as well.
        inward: f32,
    },
    /// A steady push, which comes and goes.
    Wind {
        /// Which way and how hard.
        blowing: Vec3,
        /// How much it comes and goes, from 0 to 1.
        gusts: f32,
    },
}

impl ParticleForce {
    /// How this pushes something at `place`, `time` seconds in, on an effect at `middle`.
    #[must_use]
    pub fn push(&self, place: Vec3, middle: Vec3, time: f32) -> Vec3 {
        match *self {
            Self::Attract {
                at,
                strength,
                reach,
            } => {
                let toward = middle + at - place;
                let distance = toward.length();
                if distance < 1.0 || (reach > 0.0 && distance > reach) {
                    return Vec3::ZERO;
                }
                let share = if reach > 0.0 {
                    1.0 - distance / reach
                } else {
                    1.0
                };
                toward * (strength * share / distance)
            }
            Self::Vortex {
                at,
                axis,
                strength,
                inward,
            } => {
                let line = axis.normalized();
                let from = place - (middle + at);
                let out = from - line * from.dot(line);
                let distance = out.length();
                if distance < 1.0 {
                    return Vec3::ZERO;
                }
                line.cross(out) * (strength / distance) - out * (inward / distance)
            }
            Self::Wind { blowing, gusts } => {
                let wave = (time * 0.7).sin() * 0.6 + (time * 1.9 + 1.3).sin() * 0.4;
                blowing * (1.0 + wave * gusts)
            }
        }
    }
}

impl Default for ParticleEmitter {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: ParticleKind::default(),
            blend: ParticleBlend::default(),
            rate: 0.0,
            burst: 0,
            duration: 0.0,
            delay: 0.0,
            shape: EmitterShape::default(),
            life: [1.0, 1.0],
            speed: [0.0, 0.0],
            direction: Vec3::new(0.0, 0.0, 1.0),
            spread: 180.0,
            gravity: 0.0,
            drag: 0.0,
            turbulence: 0.0,
            size: Curve(vec![(0.0, 4.0)]),
            color: ColorCurve(vec![(0.0, [1.0; 4])]),
            reach: 200.0,
            gpu: false,
            on_death: None,
            image: None,
            picture: 0,
            flipbook: [1, 1],
            flip_rate: 0.0,
            shader: None,
            shader_number: 0,
            rotation: [0.0, 0.0],
            spin: [0.0, 0.0],
            stretch: 0.0,
            soft: 0.0,
            lit: false,
            collide: ParticleCollide::None,
            bounce: 0.4,
            on_hit: None,
            forces: Vec::new(),
            outward: false,
            far: 0.0,
        }
    }
}

/// An effect.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectDef {
    /// Its name, under `effects/`.
    pub name: String,
    /// Its emitters.
    pub emitters: Vec<ParticleEmitter>,
    /// Emitters that go off at set times, as seconds from its start and a name.
    pub timeline: Vec<(f32, String)>,
}

/// An effect a level has in it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EffectPoint {
    /// The effect's name.
    pub effect: String,
    /// Where it is.
    pub origin: Vec3,
    /// The model it follows, if it moves with something.
    pub model: Option<u32>,
}
