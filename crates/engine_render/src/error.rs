//! Renderer errors.

use std::fmt;

/// The message for a machine with nothing to draw on.
const NO_DEVICE: &str = "no Vulkan device can draw here";

/// A failure in the renderer.
#[derive(Debug)]
pub struct RenderError(pub(crate) String);

impl RenderError {
    /// The failure of a machine with no usable device.
    pub(crate) fn no_device() -> Self {
        Self(NO_DEVICE.to_owned())
    }

    /// Whether this means the machine has no usable device, as opposed to the
    /// renderer failing on one that works.
    #[must_use]
    pub fn is_no_device(&self) -> bool {
        self.0 == NO_DEVICE
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RenderError {}

impl From<vulkane::safe::Error> for RenderError {
    fn from(error: vulkane::safe::Error) -> Self {
        Self(error.to_string())
    }
}
