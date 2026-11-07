//! The engine's own map type. Knows nothing about any file format.
//!
//! An importer fills a [`Map`], and every other system reads it (proposal
//! 0017).
//!
//! May depend on `engine_core`. See proposal 0008.

mod alias;
mod effect;
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
mod skeleton;
mod sky;
mod sprite;
mod stencil;
mod terrain;
mod terrain_edit;
mod tile_light;
mod tree;
mod zone;

pub use alias::{AliasCorner, AliasFrame, AliasModel, AliasVertex};
pub use effect::{
    ColorCurve, Curve, EffectDef, EffectDraw, EffectPoint, EffectSettings, EmitterShape,
    GpuEmitter, ParticleBlend, ParticleCollide, ParticleEmitter, ParticleForce, ParticleKind,
    SimEmitter,
};
pub use emitter::Emitter;
pub use live::{LiveLightSource, along_path};
pub use map::{
    Animation, AttachPoint, Entity, Face, FaceLight, Look, MOVING_MESH_PREFIX, Map, MegaImage,
    MegaMaps, MeshCorner, MeshGroup, MeshInstance, MipLevel, Model, MovingMesh, PixelForm,
    SharedMesh, SharedMorph, SharedPart, TexelAxis, Texture, TextureKind,
};
pub use material::{
    AlphaMode, FieldDef, MATERIAL_SLOTS, MAX_LAYERS, Material, MaterialLayer, MetalRough,
    PAINT_MASK, SHADER_IMAGES, SHADER_PARAMS, ShaderCode, Shading, SlotImage, mip_chain,
};
pub use nav::{LINKS, NAV_WAYS, NavGrid, NavLeap, NavLeaps, NavPlace, NavStep, NavWay, Spline};
pub use nav_chunks::{CHUNK_COLUMNS, ChunkedNav, NavChunk, NavShape, SideFiles, chunks_of};
pub use occluder::{ModelOccluder, OccluderSphere, pose_occlusion, pose_spheres};
pub use pool::{Pool, PoolLook, look_around};
pub use probes::{BOUNCE_STYLE, CRISP_STYLES, LightTrack, ProbeGrid, ProbeSample};
pub use skeleton::{
    Channel, ChannelPath, Clip, HitCapsule, Interpolation, Joint, JointPose, NO_MOVE, Rows,
    Skeleton, SkeletonPlace, after, angles_of_rows, blend_poses, moved, placed_at, turned,
};
pub use sky::{Clouds, Fog, Sky};
pub use sprite::{Sprite, SpriteFacing, SpriteFrame, SpriteImage};
pub use stencil::{RayStencil, STENCIL_GRID, stencil_boxes};
pub use terrain::{
    CHUNK_CELLS, LandPaint, LandPicture, MOST_LAND_LAYERS, TerrainData, TerrainLayer, TerrainSheet,
    USUAL_CELL,
};
pub use terrain_edit::{
    TerrainBrush, TerrainStroke, copy_land, splat_land, stamp_land, stroke_land,
};
pub use tile_light::{LightTiles, RESIDENT, TileLight};
pub use tree::{ClipChild, ClipNode, Contents, Leaf, Node, NodeChild, Plane, Tree};
pub use zone::{Zone, ZonePlane, ZoneSettings};
