//! What a client can know about a weapon before the server answers, from
//! `weapons.qc` (proposal 0052). The scripts still decide every result.

use engine_core::entity::BlastRule;
use engine_core::math::Vec3;

/// `IT_GRENADE_LAUNCHER`.
const IT_GRENADE_LAUNCHER: i32 = 16;

/// `IT_ROCKET_LAUNCHER`.
const IT_ROCKET_LAUNCHER: i32 = 32;

/// A rocket's blast on the one who fired it, from `T_MissileTouch`,
/// `T_RadiusDamage` and `T_Damage` (proposal 0053).
pub const ROCKET_BLAST: BlastRule = BlastRule {
    damage: 120.0,
    reach: 160.0,
    falloff: 0.5,
    own_share: 0.5,
    knock_per_damage: 8.0,
};

/// A weapon whose shot can be drawn at once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeaponRule {
    /// What it fires.
    pub model: &'static str,
    /// What it sounds like.
    pub sound: &'static str,
    /// Seconds between shots.
    pub refire: f64,
    /// Whether what it fires bounces.
    pub bounces: bool,
}

/// The rule of the weapon with item bit `weapon`, if its shot can be drawn.
#[must_use]
pub fn weapon_rule(weapon: i32) -> Option<WeaponRule> {
    match weapon {
        IT_ROCKET_LAUNCHER => Some(WeaponRule {
            model: "progs/missile.mdl",
            sound: "weapons/sgun1.wav",
            refire: 0.8,
            bounces: false,
        }),
        IT_GRENADE_LAUNCHER => Some(WeaponRule {
            model: "progs/grenade.mdl",
            sound: "weapons/grenade.wav",
            refire: 0.6,
            bounces: true,
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
    rule: &WeaponRule,
    origin: Vec3,
    axes: [Vec3; 3],
    pitch: f32,
    seed: u16,
) -> (Vec3, Vec3) {
    let [forward, right, up] = axes;
    if rule.bounces {
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
    } else {
        (
            origin + forward * 8.0 + Vec3::new(0.0, 0.0, 16.0),
            forward * 1000.0,
        )
    }
}
