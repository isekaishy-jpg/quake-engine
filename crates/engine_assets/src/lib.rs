//! Asset loading, pack sources and file ids.
//!
//! [`FileSystem`] is an ordered search path over [`Archive`]s. It knows no
//! file formats: an archive for a game's pack format lives with that game
//! (proposal 0013).
//!
//! May depend on `engine_core`. See proposal 0008.

mod archive;
mod directory;
mod error;
mod filesystem;
mod pack;
mod path;

pub use archive::Archive;
pub use directory::DirectoryArchive;
pub use error::AssetError;
pub use filesystem::FileSystem;
pub use pack::{PackArchive, PackSummary, write_pack, write_pack_of};
pub use path::normalize_path;
