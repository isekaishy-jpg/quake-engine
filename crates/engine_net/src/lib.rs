//! Transports and messages between client and server.
//!
//! Messages always go through bytes, even in a local game, so there's one
//! code path (proposal 0024). Decoding treats its input as hostile.
//!
//! May depend on `engine_core`. See proposal 0008.

mod conditioned;
mod delta;
mod handshake;
mod identity;
mod message;
mod online;
mod transport;

pub use conditioned::{Conditioned, Conditions};
pub use delta::{apply_delta, delta_baseline, delta_from};
pub use identity::{HostIdentity, PlayerIdentity, PublicKey, host_code, short_id, tidy_code};
pub use message::{
    ClientMessage, FireCommand, MAX_NAME_BYTES, PlayerInfo, PlayerScore, ServerMessage, Snapshot,
};
pub use online::{
    DEFAULT_PORT, HostOptions, JoinOptions, NetClient, NetError, NetServer, full_address,
};
pub use transport::{Loopback, Transport, loopback};
