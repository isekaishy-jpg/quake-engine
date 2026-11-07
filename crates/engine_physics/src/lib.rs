//! Collision and physics queries.
//!
//! The backend is `box3d`, and nothing of it appears in this crate's public
//! API (proposal 0022).
//!
//! May depend on `engine_core`, `engine_map`. See proposal 0008.

mod hull;
mod land;
mod solids;
mod world;

pub use hull::enter_box;
pub use solids::{build_solids, build_world_solids};
pub use world::{Collision, CollisionWorld, DIST_EPSILON, PointContents, Trace, is_sliver};
