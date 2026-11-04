//! Application host boundary: window, input, pacing and lifecycle.
//!
//! The backend is `solapp`, and nothing of it appears in this crate's public
//! API, so the host can be replaced without touching other crates (proposal
//! 0011).
//!
//! The application implements [`PlatformApp`] and calls [`run`] on the main
//! thread. [`PlatformApp::service`] runs the simulation, including while the
//! window is minimized, [`PlatformApp::frame`] does presentation, and
//! [`PlatformApp::input`] receives input as it arrives.
//!
//! May depend on `engine_core`. See proposal 0008.

mod error;
mod host;
mod input;

pub use error::PlatformError;
pub use host::{AppError, Frame, FrameOutcome, PlatformApp, ServiceOutcome, WindowConfig, run};
pub use input::{InputEvent, MouseButton, key_name};
