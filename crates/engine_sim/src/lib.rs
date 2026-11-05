//! The server-side simulation.
//!
//! It holds entities and moves them, and never calls a script. What's due is
//! returned to the caller, which keeps script calls out of the physics loop
//! (proposal 0025).
//!
//! May depend on `engine_core`, `engine_map`, `engine_physics`, `engine_movement`. See proposal 0008.

mod flight;
mod simulation;
mod table;
mod walk;

pub use flight::{FLAG_MONSTER, FLAG_ON_GROUND, MoveKind, MoveTrace};
pub use simulation::{
    Callback, Entity, FLAG_ALWAYS_AWAKE, FLAG_ITEM, MoveType, SavedSimulation, Simulation, Solid,
};
pub use table::{EntityId, SlotTable};
pub use walk::{FLAG_FLY, FLAG_PARTIAL_GROUND, FLAG_SWIM, Target};
