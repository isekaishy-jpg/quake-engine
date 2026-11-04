//! Player movement as a pure function.
//!
//! Everything it reads is an argument, so it can run on a copy, which is what
//! prediction and the render tick do (proposal 0023). It ports `pmove.c`.
//!
//! May depend on `engine_core`, `engine_physics`. See proposal 0008.

mod mover;

pub use mover::{MoveVars, player_move};
