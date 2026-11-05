//! Quake's stats and item bits, from `quakedef.h` and `defs.qc`.

/// `STAT_HEALTH`.
pub(crate) const STAT_HEALTH: usize = 0;
/// `STAT_AMMO`.
pub(crate) const STAT_AMMO: usize = 3;
/// `STAT_ARMOR`.
pub(crate) const STAT_ARMOR: usize = 4;
/// `STAT_SHELLS`, the first of four ammo counts.
pub(crate) const STAT_SHELLS: usize = 6;
/// `STAT_ACTIVEWEAPON`.
pub(crate) const STAT_ACTIVE_WEAPON: usize = 10;
/// `STAT_TOTALSECRETS`.
pub(crate) const STAT_TOTAL_SECRETS: usize = 11;
/// `STAT_TOTALMONSTERS`.
pub(crate) const STAT_TOTAL_MONSTERS: usize = 12;
/// `STAT_SECRETS`.
pub(crate) const STAT_SECRETS: usize = 13;
/// `STAT_MONSTERS`.
pub(crate) const STAT_MONSTERS: usize = 14;
/// The items, which the original sends apart from its stats.
pub(crate) const STAT_ITEMS: usize = 15;
/// Whether this is deathmatch.
pub(crate) const STAT_DEATHMATCH: usize = 16;
/// How many stats there are.
pub(crate) const STAT_COUNT: usize = 17;

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
/// `IT_SHELLS`.
pub(crate) const IT_SHELLS: i32 = 256;
/// `IT_NAILS`.
pub(crate) const IT_NAILS: i32 = 512;
/// `IT_ROCKETS`.
pub(crate) const IT_ROCKETS: i32 = 1024;
/// `IT_CELLS`.
pub(crate) const IT_CELLS: i32 = 2048;
/// `IT_AXE`.
pub(crate) const IT_AXE: i32 = 4096;
/// `IT_ARMOR1`.
pub(crate) const IT_ARMOR1: i32 = 8192;
/// `IT_ARMOR2`.
pub(crate) const IT_ARMOR2: i32 = 16384;
/// `IT_ARMOR3`.
pub(crate) const IT_ARMOR3: i32 = 32768;
/// `IT_INVISIBILITY`.
pub(crate) const IT_INVISIBILITY: i32 = 1 << 19;
/// `IT_INVULNERABILITY`.
pub(crate) const IT_INVULNERABILITY: i32 = 1 << 20;
/// `IT_SUIT`.
pub(crate) const IT_SUIT: i32 = 1 << 21;
/// `IT_QUAD`.
pub(crate) const IT_QUAD: i32 = 1 << 22;
