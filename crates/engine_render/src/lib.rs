//! Renderer API and backend.
//!
//! The backend is `vulkane`, and nothing of it appears in this crate's public
//! API, so the renderer can be replaced without touching other crates
//! (proposal 0018).
//!
//! May depend on `engine_core`, `engine_map`. See proposal 0008.

mod alias;
mod atlas;
mod backend;
mod canvas;
mod error;
mod instances;
mod live;
mod material;
mod matrix;
mod renderer;
mod sprite;
mod streamer;
mod surface;
mod terrain;
mod visible;
mod world;

pub use alias::{EntityDraw, WeldedCorners, model_matrix, weld_corners};
pub use atlas::{Atlas, AtlasSpace};
pub use backend::{GpuTimes, half_bits};
pub use canvas::{QuadBatch, UiBatch, UiFrame, canvas_vertices};
pub use error::{NEEDED_VULKAN, RenderError, vulkan_is_new_enough};
pub use instances::{
    INSTANCE_BYTES, InstanceBatch, LevelView, MovingDraw, RestPoses, ball_in_view, choose_level,
    height_share, instance_batches, view_planes, write_instance,
};
pub use live::{
    CLUSTERS_X, CLUSTERS_Y, CLUSTERS_Z, LiveLight, MAX_LIVE_LIGHTS, MAX_SHADOW_LIGHTS,
    cluster_masks, cube_face, slice_of,
};
pub use material::{MATERIAL_PARAMS_BYTES, MATERIAL_PARAMS_STRIDE, material_params};
pub use matrix::{height_scale, view_projection};
pub use renderer::{
    CapsuleDraw, Capture, Drawables, PostSettings, Renderer, RippleHeights, ScreenCapture,
    ScreenOutput, SkyRayFloor, Statistics, SunDraw, Target, TextureFilter, USUAL_PEAK_NITS,
    USUAL_WHITE_NITS, View,
};
pub use sprite::{
    GpuParticles, LightDraw, MAX_GPU_PARTICLE_DRAWS, MAX_LIGHT_DRAWS, MAX_PARTICLE_DRAWS,
    ParticleColor, ParticleDraw, ParticleShape, ParticleSim, SimForce, SpriteDraw, ViewAxes,
    sprite_axes, sprite_push_bytes,
};
pub use streamer::TileStreamer;
pub use terrain::{GRIDS, LEVEL_REACH, TerrainBatch, chunk_ranges, grid_indices, terrain_nodes};
pub use visible::VisibleSet;
pub use world::InstanceLight;
pub use world::{
    Batch, BatchKind, ModelRange, ShownTiles, TileImage, Vertex, WorldMesh, build_world, tile_image,
};
