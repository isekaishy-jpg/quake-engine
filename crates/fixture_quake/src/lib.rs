//! Everything specific to the Quake fixture: file formats and importers.
//!
//! The engine doesn't know Quake. This crate turns Quake's formats into
//! engine types (proposal 0001).
//!
//! May depend on `engine_core`, `engine_assets` and `engine_map`. See
//! proposal 0008.

mod game;
mod pak;

pub use game::mount_game;
pub use pak::PakArchive;
