//! The public renderer.

use std::sync::Arc;

use engine_core::canvas::{Image, Quad};
use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_core::window::NativeWindow;
use engine_map::{AliasModel, Map, Sprite};

use crate::alias::EntityDraw;
use crate::backend::{Backend, Scene};
use crate::canvas::UiFrame;
use crate::error::RenderError;
use crate::sprite::{LightDraw, ParticleDraw, SpriteDraw};
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
    /// The roll in degrees.
    pub roll: f32,
    /// The horizontal field of view in degrees, at 4:3.
    pub fov: f32,
    /// Seconds of map time.
    pub time: f32,
    /// Light style brightnesses, where 1 is normal.
    pub styles: [f32; STYLE_COUNT],
    /// What the world's colours are raised to, where 1 leaves them alone.
    pub gamma: f32,
}

/// Everything in a frame besides the map itself.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drawables<'a> {
    /// Where brush models are.
    pub instances: &'a [ModelInstance],
    /// The other models.
    pub entities: &'a [EntityDraw],
    /// The sprites.
    pub sprites: &'a [SpriteDraw],
    /// The particles.
    pub particles: &'a [ParticleDraw],
    /// The dynamic lights.
    pub lights: &'a [LightDraw],
    /// What's drawn flat over all of that, in order.
    pub quads: &'a [Quad],
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
    /// Draw calls made.
    pub batches: u32,
}

/// A loaded map.
struct LoadedMap {
    /// The map, for visibility and animation.
    map: Arc<Map>,
    /// Its mesh data.
    mesh: WorldMesh,
    /// What's visible now.
    visible: VisibleSet,
}

/// How surfaces are filtered.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextureFilter {
    /// Whether texels blend, rather than showing as hard squares.
    pub smooth: bool,
    /// Whether mip levels blend into each other.
    pub trilinear: bool,
    /// How much surfaces at an angle are sharpened, or 1 for not at all.
    pub anisotropy: f32,
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
    pub fn load_map(&mut self, map: Arc<Map>) -> Result<(), RenderError> {
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

    /// Uploads a model, and returns the number to draw it by.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub fn add_model(&mut self, model: Arc<AliasModel>) -> Result<u32, RenderError> {
        self.backend.add_model(model)
    }

    /// Uploads a sprite, and returns the number to draw it by.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub fn add_sprite(&mut self, sprite: Arc<Sprite>) -> Result<u32, RenderError> {
        self.backend.add_sprite(sprite)
    }

    /// The size of the window's picture, in pixels.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        self.backend.size()
    }

    /// Uploads a flat picture, and returns the number to draw it by.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub fn add_picture(&mut self, image: &Image) -> Result<u32, RenderError> {
        self.backend
            .add_picture(image.width, image.height, &image.rgba)
    }

    /// Chooses how plain textures are filtered, and how sharply both they and
    /// materials are sampled at an angle. Materials always blend (proposal
    /// 0070).
    ///
    /// # Errors
    ///
    /// Returns an error if the device refuses a new sampler.
    pub fn set_filter(&mut self, filter: TextureFilter) -> Result<(), RenderError> {
        self.backend.set_filter(filter)
    }

    /// Sets the UI to draw over the next frames.
    pub fn set_ui(&mut self, ui: UiFrame) {
        self.backend.set_ui(ui);
    }

    /// Chooses whether frames wait for the display. Waiting paces them
    /// evenly, and not waiting shows the newest frame sooner.
    pub fn set_vsync(&mut self, vsync: bool) {
        self.backend.set_vsync(vsync);
    }

    /// Tells the renderer the window's new size.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.backend.resize(width, height);
    }

    /// Updates the visible set for `view`, and prepares the scene.
    fn scene<'a>(
        loaded: Option<&'a mut LoadedMap>,
        view: &View,
        drawables: Drawables<'a>,
    ) -> Option<Scene<'a>> {
        let loaded = loaded?;
        let changed = loaded
            .visible
            .update(&loaded.map, &loaded.mesh, view.origin);
        Some(Scene {
            map: &loaded.map,
            mesh: &loaded.mesh,
            visible: &loaded.visible,
            drawables,
            changed,
        })
    }

    /// Draws `view` to the window.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub fn draw(&mut self, view: &View, drawables: Drawables<'_>) -> Result<(), RenderError> {
        let scene = Self::scene(self.loaded.as_mut(), view, drawables);
        self.backend.draw(view, scene.as_ref(), drawables.quads)
    }

    /// Draws `view` off-screen and returns the pixels.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub fn capture(
        &mut self,
        view: &View,
        drawables: Drawables<'_>,
        width: u32,
        height: u32,
    ) -> Result<Capture, RenderError> {
        let scene = Self::scene(self.loaded.as_mut(), view, drawables);
        self.backend
            .capture(view, scene.as_ref(), drawables.quads, width, height)
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
