//! Compiling: from a map document to what the engine runs (proposal 0067).
//!
//! May depend on `engine_core`, `engine_format`, `engine_map`. See proposal
//! 0008.

mod bake;
mod build;
mod error;
mod geometry;
mod light;
mod material;
mod source;
mod texture;
mod tree;

pub use bake::{apply_bake, source_hash, write_bake};
pub use build::{TextureSource, compile, compile_text};
pub use error::CompileError;
pub use geometry::{Plane3, Point};
pub use light::{BakeInput, ImageReader, bake_light};
pub use material::MaterialLibrary;
pub use source::{Brush, BrushFace, EntitySource, SkySource, WorkingMap, read_map};
pub use texture::placeholder_texture;
