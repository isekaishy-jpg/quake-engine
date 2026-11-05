//! Documents: the engine's own format, as text while working and binary once
//! compiled (proposal 0066).
//!
//! A document is a tree of blocks with typed fields. This crate gives them no
//! meaning, so what a map or a material holds is decided elsewhere.
//!
//! Depends on nothing. See proposal 0008.

mod binary;
mod document;
mod error;
mod text;

pub use binary::{from_binary, to_binary};
pub use document::{Block, CURRENT_VERSION, Document, Value};
pub use error::FormatError;
pub use text::{from_text, to_text};
