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
mod hud;
mod items;
mod mdl;
mod menu;
mod pak;
mod quit_messages;
mod scripts;
mod spr;
mod texture;
mod wad;
mod wav;
mod weapon_rules;

pub use bsp::{MapError, SpawnPoint, import_map, parse_entities, spawn_point, without_stray_tail};
pub use game::{mount_game, mount_music};
pub use hud::{Hud, Intermission, Score};
pub use items::parse_item_bsp;
pub use mdl::{parse_colored_mdl, parse_mdl};
pub use menu::{Menu, MenuHost, SAVE_SLOTS, Screen};
pub use pak::PakArchive;
pub use scripts::SCRIPTS;
pub use spr::parse_spr;
pub use texture::{Palette, texture_kind};
pub use wad::{Wad, load_picture, parse_picture};
pub use wav::parse_wav;
pub use weapon_rules::{ROCKET_BLAST, ShotModel, WeaponRule, shot_launch, weapon_rule};
