//! The public renderer.

use engine_core::math::Vec3;
use engine_core::window::NativeWindow;
use engine_map::Map;

use crate::backend::{Backend, Scene};
use crate::error::RenderError;
use crate::visible::VisibleSet;
use crate::world::{WorldMesh, build_world};

/// How many light styles a view carries.
const STYLE_COUNT: usize = 64;

/// Where a renderer draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Into a window.
    Window(NativeWindow),
    /// Nowhere but [`Renderer::capture`].
    Offscreen,
}

/// What to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    /// The eye position.
    pub origin: Vec3,
    /// The yaw in degrees.
    pub yaw: f32,
    /// The pitch in degrees. Positive looks down.
    pub pitch: f32,
    /// The horizontal field of view in degrees, at 4:3.
    pub fov: f32,
    /// Seconds of map time.
    pub time: f32,
    /// Light style brightnesses, where 1 is normal.
    pub styles: [f32; STYLE_COUNT],
}

/// Pixels read back from the renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Four bytes a pixel, in RGBA order.
    pub rgba: Vec<u8>,
}

/// Counts from the last drawn view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Statistics {
    /// Triangles in the map.
    pub total_triangles: u32,
    /// Triangles drawn.
    pub drawn_triangles: u32,
    /// The GPU's name is reported separately by [`Renderer::device_name`].
    pub batches: u32,
}

/// A loaded map.
struct LoadedMap {
    /// The map, for visibility and animation.
    map: Map,
    /// Its mesh data.
    mesh: WorldMesh,
    /// What's visible now.
    visible: VisibleSet,
}

/// The renderer.
pub struct Renderer {
    /// The Vulkan side.
    backend: Backend,
    /// The loaded map, if any.
    loaded: Option<LoadedMap>,
}

impl Renderer {
    /// A renderer for `target`, with a first size for a window.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if no suitable device is found, or setup fails.
    pub fn new(target: Target, size: (u32, u32)) -> Result<Self, RenderError> {
        Ok(Self {
            backend: Backend::new(target, size)?,
            loaded: None,
        })
    }

    /// The GPU in use.
    #[must_use]
    pub fn device_name(&self) -> &str {
        self.backend.device_name()
    }

    /// Replaces the loaded map.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the map's data can't be built or uploaded.
    pub fn load_map(&mut self, map: Map) -> Result<(), RenderError> {
        let mesh =
            build_world(&map).ok_or_else(|| RenderError("map's light won't fit".to_owned()))?;
        self.backend.load_map(&map, &mesh)?;
        self.loaded = Some(LoadedMap {
            map,
            mesh,
            visible: VisibleSet::default(),
        });
        Ok(())
    }

    /// Tells the renderer the window's new size.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.backend.resize(width, height);
    }

    /// Updates the visible set for `view`, and prepares the scene.
    fn scene<'a>(loaded: Option<&'a mut LoadedMap>, view: &View) -> Option<Scene<'a>> {
        let loaded = loaded?;
        let changed = loaded
            .visible
            .update(&loaded.map, &loaded.mesh, view.origin);
        Some(Scene {
            map: &loaded.map,
            mesh: &loaded.mesh,
            visible: &loaded.visible,
            changed,
        })
    }

    /// Draws `view` to the window.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub fn draw(&mut self, view: &View) -> Result<(), RenderError> {
        let scene = Self::scene(self.loaded.as_mut(), view);
        self.backend.draw(view, scene.as_ref())
    }

    /// Draws `view` off-screen and returns the pixels.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub fn capture(
        &mut self,
        view: &View,
        width: u32,
        height: u32,
    ) -> Result<Capture, RenderError> {
        let scene = Self::scene(self.loaded.as_mut(), view);
        self.backend.capture(view, scene.as_ref(), width, height)
    }

    /// Counts for the current visible set.
    #[must_use]
    pub fn statistics(&self) -> Statistics {
        self.loaded
            .as_ref()
            .map_or_else(Statistics::default, |loaded| {
                let total: usize = loaded.mesh.face_indices.iter().map(Vec::len).sum();
                Statistics {
                    total_triangles: u32::try_from(total / 3).unwrap_or(u32::MAX),
                    drawn_triangles: u32::try_from(loaded.visible.indices.len() / 3)
                        .unwrap_or(u32::MAX),
                    batches: u32::try_from(
                        loaded.visible.ranges.iter().filter(|r| r.1 > 0).count(),
                    )
                    .unwrap_or(u32::MAX),
                }
            })
    }
}
