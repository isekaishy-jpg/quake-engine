//! The engine's own map type. Knows nothing about any file format.
//!
//! An importer fills a [`Map`], and every other system reads it (proposal
//! 0017).
//!
//! May depend on `engine_core`. See proposal 0008.

mod alias;
mod light;
mod map;
mod tree;

pub use alias::{AliasCorner, AliasFrame, AliasModel, AliasVertex};
pub use map::{
    Animation, Entity, Face, FaceLight, Map, MipLevel, Model, TexelAxis, Texture, TextureKind,
};
pub use tree::{ClipChild, ClipNode, Contents, Leaf, Node, NodeChild, Plane, Tree};
