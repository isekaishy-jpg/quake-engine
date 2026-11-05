//! The engine's own map type. Knows nothing about any file format.
//!
//! An importer fills a [`Map`], and every other system reads it (proposal
//! 0017).
//!
//! May depend on `engine_core`. See proposal 0008.

mod alias;
mod light;
mod map;
mod material;
mod probes;
mod sky;
mod sprite;
mod tree;

pub use alias::{AliasCorner, AliasFrame, AliasModel, AliasVertex};
pub use map::{
    Animation, Entity, Face, FaceLight, Map, MipLevel, Model, TexelAxis, Texture, TextureKind,
};
pub use material::{
    AlphaMode, MATERIAL_SLOTS, MAX_LAYERS, Material, MaterialLayer, Shading, SlotImage, mip_chain,
};
pub use probes::{BOUNCE_STYLE, ProbeGrid, ProbeSample};
pub use sky::{Clouds, Fog, Sky};
pub use sprite::{Sprite, SpriteFacing, SpriteFrame, SpriteImage};
pub use tree::{ClipChild, ClipNode, Contents, Leaf, Node, NodeChild, Plane, Tree};
