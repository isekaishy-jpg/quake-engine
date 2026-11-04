//! Channels, spatial sound and music, over a private backend.
//!
//! The rules of who replaces what and how far a sound carries come from the
//! original, and everything else is free to do better (proposal 0029).
//!
//! May depend on `engine_core`. See proposal 0008.

mod error;
mod mixer;
mod output;
mod spatial;

pub use error::AudioError;
pub use mixer::{Audio, ClipId, PlayingSound};
pub use spatial::{Listener, amplitude_and_pan, stereo_volumes};
