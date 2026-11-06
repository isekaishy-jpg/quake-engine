//! The engine's own map type. Knows nothing about any file format.
//!
//! An importer fills a [`Map`], and every other system reads it (proposal
//! 0017).
//!
//! May depend on `engine_core`. See proposal 0008.

mod alias;
mod emitter;
mod light;
mod live;
mod map;
mod material;
mod nav;
mod nav_chunks;
mod occluder;
mod pool;
mod probes;
mod sky;
mod sprite;
mod stencil;
mod tile_light;
mod tree;

pub use alias::{AliasCorner, AliasFrame, AliasModel, AliasVertex};
pub use emitter::Emitter;
pub use live::{LiveLightSource, along_path};
pub use map::{
    Animation, Entity, Face, FaceLight, Look, Map, MegaImage, MegaMaps, MeshGroup, MipLevel, Model,
    PixelForm, TexelAxis, Texture, TextureKind,
};
pub use material::{
    AlphaMode, MATERIAL_SLOTS, MAX_LAYERS, Material, MaterialLayer, MetalRough, PAINT_MASK,
    SHADER_IMAGES, SHADER_PARAMS, ShaderCode, Shading, SlotImage, mip_chain,
};
pub use nav::{LINKS, NAV_WAYS, NavGrid, NavPlace, Spline};
pub use nav_chunks::{CHUNK_COLUMNS, ChunkedNav, NavChunk, NavShape, SideFiles, chunks_of};
pub use occluder::{ModelOccluder, OccluderSphere, pose_occlusion, pose_spheres};
pub use pool::{Pool, PoolLook};
pub use probes::{BOUNCE_STYLE, CRISP_STYLE, LightTrack, ProbeGrid, ProbeSample};
pub use sky::{Clouds, Fog, Sky};
pub use sprite::{Sprite, SpriteFacing, SpriteFrame, SpriteImage};
pub use stencil::{RayStencil, STENCIL_GRID, stencil_boxes};
pub use tile_light::{LightTiles, RESIDENT, TileLight};
pub use tree::{ClipChild, ClipNode, Contents, Leaf, Node, NodeChild, Plane, Tree};
