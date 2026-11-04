//! The engine's own map type. Knows nothing about any file format.
//!
//! An importer fills a [`Map`], and every other system reads it (proposal
//! 0017).
//!
//! May depend on `engine_core`. See proposal 0008.

mod map;
mod tree;

pub use map::{
    Animation, Entity, Face, FaceLight, Map, MipLevel, Model, TexelAxis, Texture, TextureKind,
};
pub use tree::{Contents, Leaf, Node, NodeChild, Plane, Tree};
