//! What the client shows of what happens: temporary entities, trails, dynamic
//! lights and beams, from `cl_tent.c` and `CL_RelinkEntities` (proposal 0036).
//!
//! Everything that knows the original's numbers and names is in here, so a
//! game with other effects replaces this file and keeps [`Particles`].

use std::collections::HashMap;

use engine_core::entity::Effect;
use engine_core::math::Vec3;
use engine_core::sound::SoundStart;

use crate::particles::{Particles, Trail};

/// The most dynamic lights at once.
pub const MAX_LIGHTS: usize = 32;

/// The most lightning bolts at once.
const MAX_BEAMS: usize = 24;

/// The most pieces of beam to draw.
const MAX_BEAM_PIECES: usize = 256;

/// How long a beam lasts without being sent again.
const BEAM_SECONDS: f32 = 0.2;

/// How far apart a beam's pieces are.
const BEAM_PIECE_LENGTH: f32 = 30.0;

/// The entity effect bit for a muzzle flash.
const EFFECT_MUZZLE_FLASH: u8 = 2;

/// The entity effect bit for a bright light.
const EFFECT_BRIGHT_LIGHT: u8 = 4;

/// The entity effect bit for a dim light.
const EFFECT_DIM_LIGHT: u8 = 8;

/// Model flags: a rocket's trail and light.
const FLAG_ROCKET: u32 = 1;
/// Model flags: a grenade's smoke.
const FLAG_GRENADE: u32 = 2;
/// Model flags: a gib's blood.
const FLAG_GIB: u32 = 4;
/// Model flags: a wizard's spit.
const FLAG_TRACER: u32 = 16;
/// Model flags: a zombie gib's blood.
const FLAG_ZOMBIE_GIB: u32 = 32;
/// Model flags: a death knight's spit.
const FLAG_TRACER2: u32 = 64;
/// Model flags: a vore's ball.
const FLAG_TRACER3: u32 = 128;

/// The entity id of a sound that comes from no entity.
const NO_ENTITY: u64 = u64::MAX;

/// A light that comes and goes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DynamicLight {
    /// Where it is.
    pub origin: Vec3,
    /// How far it reaches.
    pub radius: f32,
    /// The least light it adds, cutting off its edge.
    pub min_light: f32,
    /// Who it belongs to, so it can be moved. Zero is nobody.
    key: u64,
    /// When it goes.
    die: f32,
    /// How fast its radius shrinks, in units a second.
    decay: f32,
}

/// An entity as effects see it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EffectSource {
    /// Who it is.
    pub id: u64,
    /// Where it's drawn.
    pub origin: Vec3,
    /// Its pitch, yaw and roll.
    pub angles: Vec3,
    /// Its effect bits.
    pub effects: u8,
    /// Its model's flags.
    pub model_flags: u32,
}

/// A lightning bolt.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Beam {
    /// The entity it comes from.
    entity: u64,
    /// The model of each piece.
    model: &'static str,
    /// Where it starts.
    start: Vec3,
    /// Where it ends.
    end: Vec3,
    /// When it goes.
    die: f32,
}

/// One piece of a beam, to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeamPiece {
    /// The model's name.
    pub model: &'static str,
    /// Where it is.
    pub origin: Vec3,
    /// Its pitch, yaw and roll.
    pub angles: Vec3,
}

/// Where a trail has got to.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TrailState {
    /// Where its entity was last seen.
    last: Vec3,
    /// How far it's gone since its last drop.
    carried: f32,
}

/// The colour of the box a shot was judged against, which is red.
const REWOUND_COLOR: u8 = 72;

/// The colour of the box on the shooter's screen, which is green.
const SEEN_COLOR: u8 = 56;

/// The low corner of a player's box.
const SEEN_BOX_LOW: Vec3 = Vec3::new(-16.0, -16.0, -24.0);

/// The high corner of a player's box.
const SEEN_BOX_HIGH: Vec3 = Vec3::new(16.0, 16.0, 32.0);

/// Everything the client shows that the server doesn't track.
#[derive(Clone, Debug, Default)]
pub struct Effects {
    /// The particles.
    particles: Particles,
    /// The lights, with unused slots among them.
    lights: [DynamicLight; MAX_LIGHTS],
    /// The beams.
    beams: Vec<Beam>,
    /// Each moving thing's trail.
    trails: HashMap<u64, TrailState>,
    /// Sounds started here, waiting to be played.
    sounds: Vec<SoundStart>,
    /// Seconds on the same clock as [`Effects::update`].
    time: f32,
}

/// The forward direction of `angles`, from `AngleVectors`.
fn forward(angles: Vec3) -> Vec3 {
    let (pitch_sin, pitch_cos) = angles.x.to_radians().sin_cos();
    let (yaw_sin, yaw_cos) = angles.y.to_radians().sin_cos();
    Vec3::new(pitch_cos * yaw_cos, pitch_cos * yaw_sin, -pitch_sin)
}

/// The pitch and yaw that look along `direction`, from `CL_UpdateTEnts`.
fn beam_angles(direction: Vec3) -> (f32, f32) {
    if direction.x == 0.0 && direction.y == 0.0 {
        let pitch = if direction.z > 0.0 { 90.0 } else { 270.0 };
        return (pitch, 0.0);
    }
    let yaw = direction.y.atan2(direction.x).to_degrees();
    let flat = direction.x.hypot(direction.y);
    let pitch = direction.z.atan2(flat).to_degrees();
    (pitch.rem_euclid(360.0), yaw.rem_euclid(360.0))
}

impl Effects {
    /// The particles to draw.
    #[must_use]
    pub fn particles(&self) -> &Particles {
        &self.particles
    }

    /// The lights shining now.
    pub fn lights(&self) -> impl Iterator<Item = &DynamicLight> {
        self.lights
            .iter()
            .filter(|light| light.radius > 0.0 && light.die >= self.time)
    }

    /// Takes the sounds effects have started.
    pub fn take_sounds(&mut self) -> Vec<SoundStart> {
        std::mem::take(&mut self.sounds)
    }

    /// `CL_AllocDlight`: the slot for `key`, cleared. Key 0 always takes a free
    /// slot, and with none free the first is reused.
    fn light_slot(&mut self, key: u64) -> &mut DynamicLight {
        let same = (key != 0)
            .then(|| self.lights.iter().position(|light| light.key == key))
            .flatten();
        let free = || {
            self.lights
                .iter()
                .position(|light| light.die < self.time || light.radius <= 0.0)
        };
        let at = same.or_else(free).unwrap_or(0);
        let light = &mut self.lights[at];
        *light = DynamicLight {
            key,
            ..DynamicLight::default()
        };
        light
    }

    /// Starts a sound at `origin`, as `S_StartSound (-1, 0, ...)`.
    fn sound(&mut self, name: &str, origin: Vec3) {
        self.sounds.push(SoundStart {
            entity: NO_ENTITY,
            channel: 0,
            origin,
            volume: 1.0,
            attenuation: 1.0,
            own: false,
            name: name.to_owned(),
        });
    }

    /// The sound of something hitting a wall: usually a tink, and sometimes a
    /// ricochet.
    fn ricochet(&mut self, origin: Vec3) {
        if !self.particles.rand().is_multiple_of(5) {
            self.sound("weapons/tink1.wav", origin);
        } else {
            let name = match self.particles.rand() & 3 {
                1 => "weapons/ric1.wav",
                2 => "weapons/ric2.wav",
                _ => "weapons/ric3.wav",
            };
            self.sound(name, origin);
        }
    }

    /// Lights up an explosion.
    fn explosion_light(&mut self, origin: Vec3) {
        let die = self.time + 0.5;
        let light = self.light_slot(0);
        light.origin = origin;
        light.radius = 350.0;
        light.die = die;
        light.decay = 300.0;
    }

    /// `CL_ParseBeam`: a beam replaces the one from the same entity.
    fn beam(&mut self, entity: u64, model: &'static str, start: Vec3, end: Vec3) {
        let die = self.time + BEAM_SECONDS;
        let beam = Beam {
            entity,
            model,
            start,
            end,
            die,
        };
        if let Some(same) = self.beams.iter_mut().find(|beam| beam.entity == entity) {
            *same = beam;
        } else if self.beams.len() < MAX_BEAMS {
            self.beams.push(beam);
        }
    }

    /// `CL_ParseTEnt`: starts one of the original's temporary entities.
    fn temp(&mut self, kind: u8, origin: Vec3, end: Vec3, entity: u64) {
        match kind {
            0 => {
                self.particles.puff(origin, Vec3::ZERO, 0, 10);
                self.ricochet(origin);
            }
            1 => {
                self.particles.puff(origin, Vec3::ZERO, 0, 20);
                self.ricochet(origin);
            }
            2 => self.particles.puff(origin, Vec3::ZERO, 0, 20),
            3 => {
                self.particles.explosion(origin);
                self.explosion_light(origin);
                self.sound("weapons/r_exp3.wav", origin);
            }
            4 => {
                self.particles.blob_explosion(origin);
                self.sound("weapons/r_exp3.wav", origin);
            }
            5 => self.beam(entity, "progs/bolt.mdl", origin, end),
            6 => self.beam(entity, "progs/bolt2.mdl", origin, end),
            7 => {
                self.particles.puff(origin, Vec3::ZERO, 20, 30);
                self.sound("wizard/hit.wav", origin);
            }
            8 => {
                self.particles.puff(origin, Vec3::ZERO, 226, 20);
                self.sound("hknight/hit.wav", origin);
            }
            9 => self.beam(entity, "progs/bolt3.mdl", origin, end),
            10 => self.particles.lava_splash(origin),
            11 => self.particles.teleport_splash(origin),
            _ => {}
        }
    }

    /// Shows the box of a player at `origin`, as this client has them on
    /// screen, to compare with the one the server judged a shot against.
    pub fn show_seen_box(&mut self, origin: Vec3) {
        let (low, high) = (origin + SEEN_BOX_LOW, origin + SEEN_BOX_HIGH);
        self.particles.outline(low, high, SEEN_COLOR);
    }

    /// Starts something the server sent.
    pub fn start(&mut self, effect: &Effect) {
        match *effect {
            Effect::Temp {
                kind,
                origin,
                end,
                entity,
            } => self.temp(kind, origin, end, entity),
            // The client's own flights follow these.
            Effect::Launch(_) => {}
            Effect::Outline { low, high } => self.particles.outline(low, high, REWOUND_COLOR),
            Effect::Particles {
                origin,
                direction,
                color,
                count,
            } => {
                // The original sends 1024 as 255.
                let count = if count == u8::MAX {
                    1024
                } else {
                    u32::from(count)
                };
                self.particles.puff(origin, direction, color, count);
            }
        }
    }

    /// The lights `source` carries, from `CL_RelinkEntities`.
    #[expect(clippy::cast_precision_loss, reason = "the random part is below 32")]
    fn entity_lights(&mut self, source: &EffectSource) {
        let key = source.id.wrapping_add(1);
        let time = self.time;
        if source.effects & EFFECT_MUZZLE_FLASH != 0 {
            let radius = 200.0 + (self.particles.rand() & 31) as f32;
            let origin = source.origin + Vec3::new(0.0, 0.0, 16.0) + forward(source.angles) * 18.0;
            let light = self.light_slot(key);
            light.origin = origin;
            light.radius = radius;
            light.min_light = 32.0;
            light.die = time + 0.1;
        }
        if source.effects & EFFECT_BRIGHT_LIGHT != 0 {
            let radius = 400.0 + (self.particles.rand() & 31) as f32;
            let light = self.light_slot(key);
            light.origin = source.origin + Vec3::new(0.0, 0.0, 16.0);
            light.radius = radius;
            light.die = time + 0.001;
        }
        if source.effects & EFFECT_DIM_LIGHT != 0 {
            let radius = 200.0 + (self.particles.rand() & 31) as f32;
            let light = self.light_slot(key);
            light.origin = source.origin;
            light.radius = radius;
            light.die = time + 0.001;
        }
        if source.model_flags & FLAG_ROCKET != 0 {
            let light = self.light_slot(key);
            light.origin = source.origin;
            light.radius = 200.0;
            light.die = time + 0.01;
        }
    }

    /// The trail of a model with `flags`, in the original's order.
    fn trail_of(flags: u32) -> Option<Trail> {
        [
            (FLAG_GIB, Trail::Blood),
            (FLAG_ZOMBIE_GIB, Trail::SlightBlood),
            (FLAG_TRACER, Trail::Tracer),
            (FLAG_TRACER2, Trail::Tracer2),
            (FLAG_ROCKET, Trail::Rocket),
            (FLAG_GRENADE, Trail::Smoke),
            (FLAG_TRACER3, Trail::Voor),
        ]
        .into_iter()
        .find_map(|(flag, trail)| (flags & flag != 0).then_some(trail))
    }

    /// Follows entities as they're drawn this frame, for their lights and
    /// trails. An entity no longer in `sources` loses its trail.
    pub fn follow(&mut self, sources: &[EffectSource]) {
        let mut trails = HashMap::with_capacity(self.trails.len());
        for source in sources {
            self.entity_lights(source);
            let Some(kind) = Self::trail_of(source.model_flags) else {
                continue;
            };
            let mut trail = self.trails.get(&source.id).copied().unwrap_or(TrailState {
                last: source.origin,
                carried: 0.0,
            });
            self.particles
                .trail(trail.last, source.origin, kind, &mut trail.carried);
            trail.last = source.origin;
            let _previous = trails.insert(source.id, trail);
        }
        self.trails = trails;
    }

    /// Moves everything on to `time`.
    pub fn update(&mut self, time: f32) {
        let seconds = (time - self.time).max(0.0);
        self.time = time;
        self.particles.update(time);
        for light in &mut self.lights {
            if light.die >= time {
                light.radius = (light.radius - seconds * light.decay).max(0.0);
            }
        }
        self.beams.retain(|beam| beam.die >= time);
    }

    /// The pieces of every beam, from `CL_UpdateTEnts`. A beam from `player`
    /// starts at `player_origin`, so it stays with the view.
    #[expect(clippy::cast_precision_loss, reason = "a roll is far below 2^24")]
    pub fn beam_pieces(&mut self, player: u64, player_origin: Vec3) -> Vec<BeamPiece> {
        let mut pieces = Vec::new();
        for index in 0..self.beams.len() {
            let beam = &mut self.beams[index];
            if beam.entity == player {
                beam.start = player_origin;
            }
            let beam = *beam;
            let path = beam.end - beam.start;
            let (pitch, yaw) = beam_angles(path);
            let direction = path.normalized();
            let mut left = path.length();
            let mut origin = beam.start;
            while left > 0.0 && pieces.len() < MAX_BEAM_PIECES {
                let roll = (self.particles.rand() % 360) as f32;
                pieces.push(BeamPiece {
                    model: beam.model,
                    origin,
                    angles: Vec3::new(pitch, yaw, roll),
                });
                origin = origin + direction * BEAM_PIECE_LENGTH;
                left -= BEAM_PIECE_LENGTH;
            }
        }
        pieces
    }
}
