//! Commands, cvars and the command buffer.
//!
//! The console knows no game: an application registers its commands and
//! passes its own context to [`Console::execute`] (proposal 0012). Parsing
//! follows Quake's `cmd.c`, which its config files rely on.
//!
//! May depend on `engine_core`. See proposal 0008.

mod console;
mod cvar;
mod tokenize;

pub use console::{Args, Console, Handler};
pub use cvar::{Cvar, CvarFlags};
pub use tokenize::{split_commands, tokenize};
