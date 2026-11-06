//! The Luau host and the engine API scripts see.
//!
//! The backend is `mlua`, and nothing of it appears in this crate's public
//! API. The host knows no game: sources arrive as a list (proposal 0025).
//!
//! May depend on `engine_core`, `engine_map`, `engine_sim`. See proposal 0008.

mod builder;
mod game;
mod save;
mod ui_script;

pub use builder::{ScriptValue, run_builder};
pub use game::{Game, GameRules, Outbox, ScriptError};
pub use save::{SavedEntity, SavedScripts, SavedValue};
pub use ui_script::{UiCommand, UiScript, UiState};
