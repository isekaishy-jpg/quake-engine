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
mod mdl;
mod pak;
mod scripts;
mod texture;
mod wav;

pub use bsp::{MapError, SpawnPoint, import_map, parse_entities, spawn_point, without_stray_tail};
pub use game::{mount_game, mount_music};
pub use items::parse_item_bsp;
pub use mdl::parse_mdl;
pub use pak::PakArchive;
pub use scripts::SCRIPTS;
pub use texture::{Palette, texture_kind};
pub use wav::parse_wav;
