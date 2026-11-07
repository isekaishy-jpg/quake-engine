//! Quake's stats and item bits, from `quakedef.h` and `defs.qc`.

/// `STAT_HEALTH`.
pub const STAT_HEALTH: usize = 0;
/// `STAT_AMMO`.
pub const STAT_AMMO: usize = 3;
/// `STAT_ARMOR`.
pub const STAT_ARMOR: usize = 4;
/// `STAT_SHELLS`, the first of four ammo counts.
pub(crate) const STAT_SHELLS: usize = 6;
/// `STAT_ACTIVEWEAPON`.
pub const STAT_ACTIVE_WEAPON: usize = 10;
/// `STAT_TOTALSECRETS`.
pub(crate) const STAT_TOTAL_SECRETS: usize = 11;
/// `STAT_TOTALMONSTERS`.
pub const STAT_TOTAL_MONSTERS: usize = 12;
/// `STAT_SECRETS`.
pub(crate) const STAT_SECRETS: usize = 13;
/// `STAT_MONSTERS`.
pub const STAT_MONSTERS: usize = 14;
/// The items, which the original sends apart from its stats.
pub const STAT_ITEMS: usize = 15;
/// Whether this is deathmatch.
pub(crate) const STAT_DEATHMATCH: usize = 16;
/// How many stats there are.
pub const STAT_COUNT: usize = 17;

/// `IT_SHOTGUN`, the first of seven weapons.
pub(crate) const IT_SHOTGUN: i32 = 1;
/// `IT_SUPER_SHOTGUN`.
pub(crate) const IT_SUPER_SHOTGUN: i32 = 2;
/// `IT_NAILGUN`.
pub(crate) const IT_NAILGUN: i32 = 4;
/// `IT_SUPER_NAILGUN`.
pub(crate) const IT_SUPER_NAILGUN: i32 = 8;
/// `IT_GRENADE_LAUNCHER`.
pub(crate) const IT_GRENADE_LAUNCHER: i32 = 16;
/// `IT_ROCKET_LAUNCHER`.
pub(crate) const IT_ROCKET_LAUNCHER: i32 = 32;
/// `IT_LIGHTNING`.
pub(crate) const IT_LIGHTNING: i32 = 64;
/// `IT_AXE`.
pub(crate) const IT_AXE: i32 = 4096;
/// `IT_INVISIBILITY`.
pub(crate) const IT_INVISIBILITY: i32 = 1 << 19;
/// `IT_INVULNERABILITY`.
pub(crate) const IT_INVULNERABILITY: i32 = 1 << 20;
/// `IT_SUIT`.
pub(crate) const IT_SUIT: i32 = 1 << 21;
/// `IT_QUAD`.
pub(crate) const IT_QUAD: i32 = 1 << 22;

/// The stats, by what the UI script calls them.
pub const STAT_NAMES: [(&str, usize); 12] = [
    ("health", STAT_HEALTH),
    ("ammo", STAT_AMMO),
    ("shells", STAT_SHELLS),
    ("nails", STAT_SHELLS + 1),
    ("rockets", STAT_SHELLS + 2),
    ("cells", STAT_SHELLS + 3),
    ("weapon", STAT_ACTIVE_WEAPON),
    ("total_secrets", STAT_TOTAL_SECRETS),
    ("total_monsters", STAT_TOTAL_MONSTERS),
    ("secrets", STAT_SECRETS),
    ("monsters", STAT_MONSTERS),
    ("deathmatch", STAT_DEATHMATCH),
];

/// What's held, by what the UI script calls it.
pub const ITEM_NAMES: [(&str, i32); 6] = [
    ("key1", 1 << 17),
    ("key2", 1 << 18),
    ("invisibility", IT_INVISIBILITY),
    ("invulnerability", IT_INVULNERABILITY),
    ("suit", IT_SUIT),
    ("quad", IT_QUAD),
];
