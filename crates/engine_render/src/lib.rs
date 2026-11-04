//! Renderer API and backend.
//!
//! The backend is `vulkane`, and nothing of it appears in this crate's public
//! API, so the renderer can be replaced without touching other crates
//! (proposal 0018).
//!
//! May depend on `engine_core`, `engine_map`. See proposal 0008.

mod atlas;
mod backend;
mod error;
mod matrix;
mod overlay;
mod renderer;
mod visible;
mod world;

pub use atlas::Atlas;
pub use error::RenderError;
pub use matrix::view_projection;
pub use overlay::overlay_vertices;
pub use renderer::{Capture, Renderer, Statistics, Target, View};
pub use visible::VisibleSet;
pub use world::{Batch, BatchKind, Vertex, WorldMesh, build_world};
