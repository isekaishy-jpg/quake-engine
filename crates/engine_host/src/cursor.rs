//! Prepared cursor validation and the existing winit native cursor owner.

use winit::event_loop::ActiveEventLoop;
use winit::platform::windows::ActiveEventLoopExtWindows;
use winit::window::{CustomCursor, CustomCursorSource};

use crate::{Error, HostId, NativeOperation, Rejected};

/// Prepared packed straight-alpha RGBA8 pixels. No image transformations occur.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedCursor {
    /// Row-major pixels with exactly four bytes per pixel and no row padding.
    pub rgba: Vec<u8>,
    /// Nonzero image width, within the backend's u16 image contract.
    pub width: u16,
    /// Nonzero image height, within the backend's u16 image contract.
    pub height: u16,
    /// Horizontal hotspot inside the image.
    pub hotspot_x: u16,
    /// Vertical hotspot inside the image.
    pub hotspot_y: u16,
}

impl PreparedCursor {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.width == 0 || self.height == 0 {
            return Err(Error::InvalidInput("cursor dimensions must be nonzero"));
        }
        let bytes = usize::from(self.width)
            .checked_mul(usize::from(self.height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(Error::InvalidInput("cursor byte count overflows"))?;
        if self.rgba.len() != bytes {
            return Err(Error::InvalidInput(
                "cursor RGBA byte count differs from dimensions",
            ));
        }
        if self.hotspot_x >= self.width || self.hotspot_y >= self.height {
            return Err(Error::InvalidInput("cursor hotspot is outside the image"));
        }
        Ok(())
    }

    fn source(&self) -> Result<CustomCursorSource, Error> {
        self.validate()?;
        // Keep the original prepared input intact until native creation succeeds.
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(self.rgba.len())
            .map_err(|_| Error::AllocationFailed)?;
        pixels.extend_from_slice(&self.rgba);
        CustomCursor::from_rgba(
            pixels,
            self.width,
            self.height,
            self.hotspot_x,
            self.hotspot_y,
        )
        .map_err(|_| Error::InvalidInput("cursor image rejected by backend validation"))
    }
}

/// A successfully created cursor resource. Cloning retains the same native owner.
#[derive(Clone, Debug)]
pub struct Cursor {
    pub(crate) host: HostId,
    pub(crate) inner: CustomCursor,
}

/// Supported existing system cursor identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SystemCursor {
    /// Standard arrow.
    Arrow,
    /// Text insertion caret.
    Text,
    /// Link or action hand.
    Hand,
    /// Crosshair.
    Crosshair,
    /// Busy indicator.
    Wait,
    /// Horizontal resize.
    ResizeHorizontal,
    /// Vertical resize.
    ResizeVertical,
}
impl SystemCursor {
    pub(crate) fn native(self) -> winit::window::CursorIcon {
        use winit::window::CursorIcon;
        match self {
            Self::Arrow => CursorIcon::Default,
            Self::Text => CursorIcon::Text,
            Self::Hand => CursorIcon::Pointer,
            Self::Crosshair => CursorIcon::Crosshair,
            Self::Wait => CursorIcon::Wait,
            Self::ResizeHorizontal => CursorIcon::EwResize,
            Self::ResizeVertical => CursorIcon::NsResize,
        }
    }
}

/// Owned window-local cursor selection.
#[derive(Clone, Debug)]
pub enum CursorSelection {
    /// Backend default arrow.
    Default,
    /// Existing system cursor.
    System(SystemCursor),
    /// Retain this host's successfully created native resource.
    Custom(Cursor),
}

pub(crate) fn create_native_cursor(
    event_loop: &ActiveEventLoop,
    host: HostId,
    input: PreparedCursor,
) -> Result<Cursor, Rejected<PreparedCursor>> {
    let result = input.source().and_then(|source| {
        event_loop
            .try_create_custom_cursor(source)
            .map(|inner| Cursor { host, inner })
            .map_err(|error| Error::Native {
                operation: NativeOperation::CreateCursor,
                message: error.to_string(),
            })
    });
    result.map_err(|error| Rejected::new(input, error))
}

#[cfg(test)]
#[path = "../tests/unit/cursors.rs"]
mod tests;
