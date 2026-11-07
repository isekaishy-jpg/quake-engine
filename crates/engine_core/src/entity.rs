//! What every system needs to know about what's in the world.

use crate::math::Vec3;

/// A hash of an effect's name, which is how a server names one.
#[must_use]
pub fn effect_id(name: &str) -> u32 {
    name.bytes().fold(0x811c_9dc5, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

/// The entity effect bit for a muzzle flash, from `EF_MUZZLEFLASH`.
pub const EFFECT_MUZZLE_FLASH: u8 = 2;

/// A brush model in the world.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ModelInstance {
    /// The model's number in the map.
    pub model: u32,
    /// How far it has moved from where the map built it.
    pub origin: Vec3,
    /// Whether it's drawn.
    pub visible: bool,
    /// Whether it blocks movement.
    pub solid: bool,
    /// Its frame. Anything but 0 shows its textures' alternates.
    pub frame: u16,
}

/// An entity the client draws, by its model.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EntityState {
    /// Who it is, across snapshots.
    pub id: u64,
    /// Its model, by the number the server gave the name.
    pub model: u16,
    /// Which frame it shows.
    pub frame: u16,
    /// Which skin it shows.
    pub skin: u8,
    /// Effects, by the original's bits.
    pub effects: u8,
    /// Where it is.
    pub origin: Vec3,
    /// Its pitch, yaw and roll in degrees.
    pub angles: Vec3,
    /// Which clip of a moving mesh it's playing, plus one, or 0 for none.
    pub clip: u16,
    /// When that began.
    pub clip_start: f32,
}

/// Something launched, whose path is fixed from here on (proposal 0052).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Projectile {
    /// Its entity.
    pub entity: u64,
    /// Who fired it.
    pub owner: u64,
    /// Where it starts.
    pub origin: Vec3,
    /// How it starts moving.
    pub velocity: Vec3,
    /// The game time it starts.
    pub time: f64,
    /// Whether it falls and bounces, rather than flying straight.
    pub bounces: bool,
    /// Whether it blows up when it first hits something.
    pub blasts: bool,
    /// How many times stronger than usual its damage is.
    pub strength: f32,
    /// Seconds until it blows up by itself, or 0 if it won't.
    pub fuse: f32,
}

/// How a shot feels to the one firing it, which a game supplies so that it can
/// be shown at once (proposal 0057).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShotFeel {
    /// How far the view kicks up, in degrees.
    pub recoil: f32,
    /// Whether the muzzle flashes.
    pub flash: bool,
    /// The first and last frames of the weapon, a tenth of a second each.
    pub frames: Option<(u16, u16)>,
    /// Whether those go round for as long as it's held.
    pub repeats: bool,
}

/// How a blast knocks the one who set it off, for predicting it (proposal
/// 0053). Games supply this. The server's numbers always win.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BlastRule {
    /// The damage at its centre.
    pub damage: f32,
    /// How far it reaches.
    pub reach: f32,
    /// How much damage is lost per unit of distance.
    pub falloff: f32,
    /// The share of the damage you take from your own.
    pub own_share: f32,
    /// The damage of a direct hit, before any strength.
    pub direct_damage: f32,
    /// How much speed a point of damage gives.
    pub knock_per_damage: f32,
    /// The stat that says what the player is carrying.
    pub carried_stat: usize,
    /// The bits of that stat that make blasts stronger.
    pub boost_bits: i32,
    /// How many times stronger.
    pub boost: f32,
}

/// A short-lived effect, which only the client knows about after it's sent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    /// One of the original's temporary entities, by its number.
    Temp {
        /// Which one.
        kind: u8,
        /// Where it is.
        origin: Vec3,
        /// Where a beam ends.
        end: Vec3,
        /// The entity a beam comes from.
        entity: u64,
    },
    /// An effect an author described, by the hash of its name.
    Named {
        /// Which one.
        effect: u32,
        /// Where it starts.
        origin: Vec3,
        /// The entity it follows, or 0 for none.
        entity: u64,
    },
    /// A puff of particles.
    Particles {
        /// Where they start.
        origin: Vec3,
        /// How they move.
        direction: Vec3,
        /// Their palette colour.
        color: u8,
        /// How many.
        count: u8,
    },
    /// Something's been launched.
    Launch(Projectile),
    /// A box to show for a moment, for seeing what a shot was judged against.
    Outline {
        /// Its low corner.
        low: Vec3,
        /// Its high corner.
        high: Vec3,
    },
}
