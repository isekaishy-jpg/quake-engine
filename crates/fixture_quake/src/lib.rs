//! Everything specific to the Quake fixture: file formats and importers.
//!
//! The engine doesn't know Quake. This crate turns Quake's formats into
//! engine types (proposal 0001).
//!
//! May depend on `engine_core`, `engine_assets` and `engine_map`. See
//! proposal 0008.

mod anorms;
mod bsp;
mod game;
mod items;
mod layout;
mod mdl;
mod pak;
mod scripts;
mod spr;
mod stats;
mod texture;
mod wad;
mod wav;
mod weapon_rules;

pub use bsp::{MapError, SpawnPoint, import_map, parse_entities, spawn_point, without_stray_tail};
pub use game::{ColouredLevels, mount_coloured_levels, mount_game, mount_music};
pub use items::parse_item_bsp;
pub use layout::{
    BACKGROUND_PICTURE, BASE_GAME, DEFAULT_CONFIG, GAME_COMMANDS, GAME_RULES, LIVE_RULES,
    LoadedLevel, PICTURE_WAD, SKY_SOUND, STARTUP_SCRIPT, TALK_SOUND, WATER_SOUND, load_level,
    load_palette, music_file,
};
pub use mdl::{parse_colored_mdl, parse_mdl};
pub use pak::PakArchive;
pub use scripts::SCRIPTS;
pub use spr::parse_spr;
pub use stats::{
    ITEM_NAMES, STAT_ACTIVE_WEAPON, STAT_AMMO, STAT_ARMOR, STAT_COUNT, STAT_HEALTH, STAT_ITEMS,
    STAT_MONSTERS, STAT_NAMES, STAT_TOTAL_MONSTERS,
};
pub use texture::{Palette, texture_kind};
pub use wad::{Wad, load_picture, parse_picture};
pub use wav::parse_wav;
pub use weapon_rules::{ROCKET_BLAST, ShotModel, WeaponRule, shot_launch, weapon_rule};
