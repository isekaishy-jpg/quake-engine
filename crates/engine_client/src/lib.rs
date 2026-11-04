//! Client state: prediction, interpolation and input commands.
//!
//! May depend on `engine_core`, `engine_console`, `engine_map`, `engine_physics`, `engine_movement`, `engine_net`. See proposal 0008.

mod bindings;
mod fly;
mod look;

pub use bindings::{Bindings, canonical_key};
pub use fly::{Buttons, FlyCamera};
pub use look::{Look, LookScale};
