//! The server-side simulation.
//!
//! It holds entities and moves them, and never calls a script. What's due is
//! returned to the caller, which keeps script calls out of the physics loop
//! (proposal 0025).
//!
//! May depend on `engine_core`, `engine_map`, `engine_physics`, `engine_movement`. See proposal 0008.

mod simulation;
mod table;

pub use simulation::{Callback, Entity, MoveType, Simulation, Solid};
pub use table::{EntityId, SlotTable};
