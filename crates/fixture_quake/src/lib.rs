//! Everything specific to the Quake fixture: file formats and importers.
//!
//! The engine doesn't know Quake. This crate turns Quake's formats into
//! engine types (proposal 0001).
//!
//! May depend on `engine_core`, `engine_assets` and `engine_map`. See
//! proposal 0008.

mod bsp;
mod game;
mod lightstyle;
mod pak;
mod texture;

pub use bsp::{MapError, SpawnPoint, import_map, spawn_point, without_stray_tail};
pub use game::mount_game;
pub use lightstyle::{LIGHT_STYLE_COUNT, light_styles};
pub use pak::PakArchive;
pub use texture::{Palette, texture_kind};
