//! Client state: prediction, interpolation and input commands.
//!
//! May depend on `engine_core`, `engine_console`, `engine_map`, `engine_physics`, `engine_movement`, `engine_net`. See proposal 0008.

mod look;

pub use look::{Look, LookScale};
