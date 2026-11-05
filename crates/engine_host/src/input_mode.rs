//! Requested input policy and last confirmed native outcomes.

use crate::{CursorSelection, Error};

/// Independent relative-motion delivery and pointer-confinement requests.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InputMode {
    /// Select this window as the raw-motion recipient while platform-focused.
    pub relative_motion: bool,
    /// Confine the pointer to the native client area when supported.
    pub confine_pointer: bool,
}

/// Requested mode and individually confirmed native state.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InputModeState {
    /// Latest request, including a request that failed.
    pub requested: InputMode,
    /// Last successful operations; relative delivery also requires platform focus.
    pub confirmed: InputMode,
    /// Last native mode failure, cleared by a completely successful request.
    pub failure: Option<Error>,
}

/// Policy for the existing backend's single raw-device registrar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawInputPolicy {
    /// Remove raw registration.
    Never,
    /// Register native devices for delivery while this application is focused.
    WhenFocused,
}

/// Global raw registration request versus the last confirmed native call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawInputState {
    /// Latest application policy request. Retirement can confirm `Never`
    /// without overwriting this request history.
    pub requested: RawInputPolicy,
    /// Last confirmed native policy; absent before `engine_host`'s first native call.
    pub confirmed: Option<RawInputPolicy>,
    /// Diagnostic from the latest failed registration attempt.
    pub failure: Option<Error>,
}
impl Default for RawInputState {
    fn default() -> Self {
        Self {
            requested: RawInputPolicy::Never,
            confirmed: None,
            failure: None,
        }
    }
}

/// Retained cursor selection and independent visibility policy.
#[derive(Clone, Debug)]
pub struct CursorState {
    /// Window-local selected resource, retained even when the pointer is elsewhere.
    pub selection: CursorSelection,
    /// Application visibility request, independent of relative-motion suppression.
    pub requested_visible: bool,
    /// Relative-motion policy currently suppresses the cursor.
    pub input_suppressed: bool,
}
impl Default for CursorState {
    fn default() -> Self {
        Self {
            selection: CursorSelection::Default,
            requested_visible: true,
            input_suppressed: false,
        }
    }
}
