//! Renderer errors.

use std::fmt;

/// A failure in the renderer.
#[derive(Debug)]
pub struct RenderError(pub(crate) String);

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
