//! Transports and messages between client and server.
//!
//! Messages always go through bytes, even in a local game, so there's one
//! code path (proposal 0024). Decoding treats its input as hostile.
//!
//! May depend on `engine_core`. See proposal 0008.

mod message;
mod transport;

pub use message::{ClientMessage, ServerMessage, Snapshot};
pub use transport::{Loopback, Transport, loopback};
