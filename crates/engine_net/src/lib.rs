//! Transports and messages between client and server.
//!
//! Messages always go through bytes, even in a local game, so there's one
//! code path (proposal 0024). Decoding treats its input as hostile.
//!
//! May depend on `engine_core`. See proposal 0008.

mod delta;
mod message;
mod transport;

pub use delta::{apply_delta, delta_baseline, delta_from};
pub use message::{ClientMessage, PlayerInfo, PlayerScore, ServerMessage, Snapshot};
pub use transport::{Loopback, Transport, loopback};
