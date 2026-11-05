//! Minimal native window startup contracts.

use crate::backend::NativeWindow;
use crate::error::Error;
use crate::identity::WindowGeneration;

/// Size in physical pixels. Window creation validates both components.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalSize {
    /// Width in physical pixels.
    pub width: u32,
    /// Height in physical pixels.
    pub height: u32,
}

/// Owned startup input for a normal windowed window.
/// Display modes and retained rendering access are later capabilities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowSpec {
    /// UTF-8 title. Embedded NUL is rejected before native mutation.
    pub title: String,
    /// Requested client size in physical pixels, before native adjustments.
    pub size: PhysicalSize,
    /// Whether the native window is initially visible.
    pub visible: bool,
}

impl Default for WindowSpec {
    fn default() -> Self {
        Self {
            title: String::from("engine_host"),
            size: PhysicalSize {
                width: 960,
                height: 540,
            },
            visible: true,
        }
    }
}

impl WindowSpec {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.title.contains('\0') {
            return Err(Error::InvalidInput("window title contains NUL"));
        }
        if self.size.width == 0
            || self.size.height == 0
            || self.size.width > i32::MAX as u32
            || self.size.height > i32::MAX as u32
        {
            return Err(Error::InvalidInput(
                "window size is outside the native range",
            ));
        }
        Ok(())
    }
}

/// Host-owned native window lifecycle. This is not renderer readiness.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowState {
    /// A native window was acquired and is retained by the host.
    Live,
    /// Admission is closed; application final access may still need it.
    Retiring,
}

pub(crate) struct WindowRecord {
    pub(crate) generation: WindowGeneration,
    pub(crate) native: NativeWindow,
    pub(crate) access: std::sync::Arc<crate::window_access::WindowAnchor>,
    pub(crate) closing: bool,
    pub(crate) display: crate::display::DisplayState,
    pub(crate) input_mode: crate::InputModeState,
    pub(crate) confinement_uncertain: bool,
    pub(crate) cursor: crate::CursorState,
}
