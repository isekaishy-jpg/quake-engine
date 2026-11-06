//! The development platform's core (proposal 0073).
//!
//! Every edit is a named operation taking and returning JSON, so the editor,
//! the command line and MCP all do the same things in the same way.
//!
//! May depend on `engine_assets`, `engine_compile`, `engine_format`,
//! `engine_map`. See proposal 0008.

mod features;
mod game_run;
mod json;
mod operations;
mod plan;
mod prefab;
mod session;

pub use game_run::GameRunner;
pub use operations::{OPERATIONS, OperationInfo, run};
pub use plan::{CELL, Plan, Square, is_plan_map};
pub use prefab::{PrefabOutput, build_prefab, prefab_names};
pub use session::Session;
