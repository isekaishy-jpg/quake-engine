//! Transports and messages between client and server.
//!
//! Messages always go through bytes, even in a local game, so there's one
//! code path (proposal 0024). Decoding treats its input as hostile.
//!
//! May depend on `engine_core`. See proposal 0008.

mod conditioned;
mod delta;
mod handshake;
mod message;
mod online;
mod transport;

pub use conditioned::{Conditioned, Conditions};
pub use delta::{apply_delta, delta_baseline, delta_from};
pub use message::{ClientMessage, FireCommand, PlayerInfo, PlayerScore, ServerMessage, Snapshot};
pub use online::{DEFAULT_PORT, NetClient, NetError, NetServer};
pub use transport::{Loopback, Transport, loopback};
