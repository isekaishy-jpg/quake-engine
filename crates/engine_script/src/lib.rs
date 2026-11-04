//! The Luau host and the engine API scripts see.
//!
//! The backend is `mlua`, and nothing of it appears in this crate's public
//! API. The host knows no game: sources arrive as a list (proposal 0025).
//!
//! May depend on `engine_core`, `engine_map`, `engine_sim`. See proposal 0008.

mod game;

pub use game::{Game, Outbox, ScriptError};
