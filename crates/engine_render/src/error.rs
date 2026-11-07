//! Renderer errors.

use std::fmt;

/// The message for a machine with nothing to draw on.
const NO_DEVICE: &str = "no Vulkan device can draw here";

/// The Vulkan a device needs, as its major and minor version.
pub const NEEDED_VULKAN: (u32, u32) = (1, 3);

/// Whether a device with this version of Vulkan will do.
#[must_use]
pub fn vulkan_is_new_enough(version: (u32, u32)) -> bool {
    version >= NEEDED_VULKAN
}

/// A failure in the renderer.
#[derive(Debug)]
pub struct RenderError(pub(crate) String);

impl RenderError {
    /// The failure of a machine with no usable device.
    pub(crate) fn no_device() -> Self {
        Self(NO_DEVICE.to_owned())
    }

    /// The failure of a machine whose only device, called `name`, has Vulkan `version` and
    /// not what the engine needs. It says which, as a player can do something about that.
    pub(crate) fn too_old(name: &str, version: (u32, u32)) -> Self {
        let (major, minor) = version;
        let (needed_major, needed_minor) = NEEDED_VULKAN;
        Self(format!(
            "{name} has Vulkan {major}.{minor}, and this needs {needed_major}.{needed_minor}. \
             Updating its driver may help."
        ))
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
