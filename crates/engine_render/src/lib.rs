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
mod live;
mod material;
mod matrix;
mod renderer;
mod sprite;
mod visible;
mod world;

pub use alias::{EntityDraw, model_matrix, pose_bytes, push_bytes, uv_bytes};
pub use atlas::Atlas;
pub use canvas::{QuadBatch, UiBatch, UiFrame, canvas_vertices};
pub use error::RenderError;
pub use live::{
    CLUSTERS_X, CLUSTERS_Y, CLUSTERS_Z, LiveLight, MAX_LIVE_LIGHTS, MAX_SHADOW_LIGHTS,
    cluster_masks, cube_face, slice_of,
};
pub use material::{MATERIAL_PARAMS_BYTES, MATERIAL_PARAMS_STRIDE, material_params};
pub use matrix::view_projection;
pub use renderer::{
    CapsuleDraw, Capture, Drawables, PostSettings, Renderer, Statistics, Target, TextureFilter,
    View,
};
pub use sprite::{
    LightDraw, MAX_LIGHT_DRAWS, MAX_PARTICLE_DRAWS, ParticleColor, ParticleDraw, SpriteDraw,
    ViewAxes, sprite_axes, sprite_push_bytes,
};
pub use visible::VisibleSet;
pub use world::{Batch, BatchKind, ModelRange, Vertex, WorldMesh, build_world};
