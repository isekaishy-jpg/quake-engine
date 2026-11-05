//! What a client can know about a weapon before the server answers, from
//! `weapons.qc` (proposal 0052). The scripts still decide every result.

use engine_core::entity::{BlastRule, ShotFeel};
use engine_core::math::Vec3;

use crate::stats::{
    IT_AXE, IT_GRENADE_LAUNCHER, IT_LIGHTNING, IT_NAILGUN, IT_QUAD, IT_ROCKET_LAUNCHER, IT_SHOTGUN,
    IT_SUPER_NAILGUN, IT_SUPER_SHOTGUN, STAT_ITEMS,
};

/// A rocket's blast on the one who fired it, from `T_MissileTouch`,
/// `T_RadiusDamage` and `T_Damage` (proposal 0053).
pub const ROCKET_BLAST: BlastRule = BlastRule {
    damage: 120.0,
    reach: 160.0,
    falloff: 0.5,
    own_share: 0.5,
    direct_damage: 110.0,
    knock_per_damage: 8.0,
    carried_stat: STAT_ITEMS,
    boost_bits: IT_QUAD,
    boost: 4.0,
};

/// What a weapon fires.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotModel {
    /// Its model.
    pub model: &'static str,
    /// Whether it bounces.
    pub bounces: bool,
    /// Whether it blows up on what it hits.
    pub blasts: bool,
    /// Seconds until it blows up by itself, or 0 if it won't.
    pub fuse: f32,
}

/// What a client can show of a weapon's shot before the server answers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponRule {
    /// What it fires, if anything flies.
    pub shot: Option<ShotModel>,
    /// What it sounds like.
    pub sound: &'static str,
    /// Whether that's only when it starts firing.
    pub sound_once: bool,
    /// Seconds between shots.
    pub refire: f64,
    /// The ammo a shot needs.
    pub ammo: i32,
    /// How it feels.
    pub feel: ShotFeel,
}

/// The six frames of `player_shot` and `player_rocket`, with a flash.
const SIX_FRAMES: (u16, u16) = (1, 6);

/// A shotgun or a launcher's feel, with a kick of `recoil`.
const fn single_shot(recoil: f32) -> ShotFeel {
    ShotFeel {
        recoil,
        flash: true,
        frames: Some(SIX_FRAMES),
        repeats: false,
    }
}

/// The feel of what fires for as long as it's held, going round to `last`.
const fn held_shot(last: u16) -> ShotFeel {
    ShotFeel {
        recoil: 2.0,
        flash: true,
        frames: Some((1, last)),
        repeats: true,
    }
}

/// The rule of the weapon with item bit `weapon`, with `ammo` left, from
/// `W_Attack` and what it calls.
#[must_use]
pub fn weapon_rule(weapon: i32, ammo: i32) -> Option<WeaponRule> {
    let nail = ShotModel {
        model: "progs/spike.mdl",
        bounces: false,
        blasts: false,
        fuse: 0.0,
    };
    let shotgun = WeaponRule {
        shot: None,
        sound: "weapons/guncock.wav",
        sound_once: false,
        refire: 0.5,
        ammo: 1,
        feel: single_shot(2.0),
    };
    let nailgun = WeaponRule {
        shot: Some(nail),
        sound: "weapons/rocket1i.wav",
        sound_once: false,
        refire: 0.1,
        ammo: 1,
        feel: held_shot(8),
    };
    match weapon {
        IT_AXE => Some(WeaponRule {
            shot: None,
            sound: "weapons/ax1.wav",
            sound_once: false,
            refire: 0.5,
            ammo: 0,
            feel: ShotFeel::default(),
        }),
        IT_SHOTGUN => Some(shotgun),
        // One shell left fires as the shotgun, but not at its pace.
        IT_SUPER_SHOTGUN if ammo == 1 => Some(WeaponRule {
            refire: 0.7,
            ..shotgun
        }),
        IT_SUPER_SHOTGUN => Some(WeaponRule {
            sound: "weapons/shotgn2.wav",
            refire: 0.7,
            ammo: 2,
            feel: single_shot(4.0),
            ..shotgun
        }),
        IT_NAILGUN => Some(nailgun),
        IT_SUPER_NAILGUN if ammo == 1 => Some(nailgun),
        IT_SUPER_NAILGUN => Some(WeaponRule {
            shot: Some(ShotModel {
                model: "progs/s_spike.mdl",
                ..nail
            }),
            sound: "weapons/spike2.wav",
            ammo: 2,
            ..nailgun
        }),
        IT_GRENADE_LAUNCHER => Some(WeaponRule {
            shot: Some(ShotModel {
                model: "progs/grenade.mdl",
                bounces: true,
                blasts: false,
                fuse: 2.5,
            }),
            sound: "weapons/grenade.wav",
            sound_once: false,
            refire: 0.6,
            ammo: 1,
            feel: single_shot(2.0),
        }),
        IT_ROCKET_LAUNCHER => Some(WeaponRule {
            shot: Some(ShotModel {
                model: "progs/missile.mdl",
                bounces: false,
                blasts: true,
                fuse: 0.0,
            }),
            sound: "weapons/sgun1.wav",
            sound_once: false,
            refire: 0.8,
            ammo: 1,
            feel: single_shot(2.0),
        }),
        IT_LIGHTNING => Some(WeaponRule {
            shot: None,
            sound: "weapons/lstart.wav",
            sound_once: true,
            refire: 0.1,
            ammo: 1,
            feel: held_shot(4),
        }),
        _ => None,
    }
}

/// One random share from -1 to 1, as `spread` in `weapons.luau` works it out.
fn spread(seed: u16, salt: u64) -> f32 {
    let hashed = ((u64::from(seed) + salt * 40503) * 2_654_435_761) % 4_294_967_296;
    #[expect(clippy::cast_precision_loss, reason = "a share needs no more")]
    let share = hashed as f32 / 2_147_483_648.0;
    share - 1.0
}

/// Where a shot starts and how it moves, for a player at `origin` looking along
/// `forward`, `right` and `up`, as `W_FireRocket` and `W_FireGrenade` do.
#[must_use]
pub fn shot_launch(
    shot: &ShotModel,
    origin: Vec3,
    axes: [Vec3; 3],
    pitch: f32,
    seed: u16,
) -> (Vec3, Vec3) {
    let [forward, right, up] = axes;
    if shot.bounces {
        let velocity = if pitch == 0.0 {
            let flat = forward * 600.0;
            Vec3::new(flat.x, flat.y, 200.0)
        } else {
            forward * 600.0
                + up * 200.0
                + right * (spread(seed, 0) * 10.0)
                + up * (spread(seed, 1) * 10.0)
        };
        (origin, velocity)
    } else if shot.blasts {
        (
            origin + forward * 8.0 + Vec3::new(0.0, 0.0, 16.0),
            forward * 1000.0,
        )
    } else {
        (origin + Vec3::new(0.0, 0.0, 16.0), forward * 1000.0)
    }
}
