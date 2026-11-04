//! Collision and physics queries.
//!
//! The backend is `box3d`, and nothing of it appears in this crate's public
//! API (proposal 0022).
//!
//! May depend on `engine_core`, `engine_map`. See proposal 0008.

mod solids;
mod world;

pub use solids::build_solids;
pub use world::{CollisionWorld, DIST_EPSILON, PointContents, Trace};
