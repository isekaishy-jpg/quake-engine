//! The public renderer.

use std::path::PathBuf;
use std::sync::Arc;

use engine_core::canvas::{Image, Quad};
use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_core::window::NativeWindow;
use engine_map::{AliasModel, Map, STENCIL_GRID, Sprite, TileLight};

use crate::alias::EntityDraw;
use crate::backend::{Backend, Scene};
use crate::canvas::UiFrame;
use crate::error::RenderError;
use crate::live::LiveLight;
use crate::sprite::{LightDraw, ParticleDraw, SpriteDraw};
use crate::visible::VisibleSet;
use crate::world::{ShownTiles, WorldMesh, build_world, detail_patches, far_patches, tile_image};

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
    /// Lights worked out this frame.
    pub live_lights: &'a [LiveLight],
    /// Where each stencil's light is now.
    pub stencil_lights: &'a [Vec3],
    /// How open each stencil's cells are.
    pub stencil_masks: &'a [[f32; STENCIL_GRID * STENCIL_GRID]],
    /// Characters shading the light.
    pub capsules: &'a [CapsuleDraw],
}

/// A character that shades what's behind it, with no shadow map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CapsuleDraw {
    /// Its middle.
    pub origin: Vec3,
    /// How wide it is.
    pub radius: f32,
    /// Which way its light comes from.
    pub toward_light: Vec3,
    /// How dark its shade is, from 0 to 1.
    pub strength: f32,
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
    /// Which tiles' light is in the atlas.
    shown: ShownTiles,
}

/// What the final pass does (proposal 0075).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PostSettings {
    /// How hard edges are smoothed: 0 not at all, 1 by a quick blend, 2 by
    /// searching along each edge for its ends.
    pub edge_smoothing: u8,
    /// Outlines: 0 for none, 1 for full, 2 for semi.
    pub outline: u8,
    /// How wide outlines are, in pixels.
    pub outline_width: f32,
    /// How far away semi-outlines fade out.
    pub outline_fade: f32,
    /// Their colour.
    pub outline_color: [f32; 3],
    /// Stencil rays: 0 for none, 1 for a box each, 2 for chunky, 3 for full.
    pub rays: u8,
    /// Whether models are outlined by a pushed-out hull.
    pub outline_hull: bool,
    /// How wide fake rounded edges are, in pixels, or 0 for none.
    pub rounded_edges: f32,
    /// How far details reach before they've faded out.
    pub detail_distance: f32,
    /// Whether moving lights cast shadows.
    pub live_shadows: bool,
    /// Whether fog reads the light in the probes.
    pub fog_grid: bool,
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
        Self::keeping_pipelines(target, size, None)
    }

    /// A renderer that keeps built pipelines in the file at `saved`, so a next
    /// run needn't build them again.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if no suitable device is found, or setup fails.
    pub fn keeping_pipelines(
        target: Target,
        size: (u32, u32),
        saved: Option<PathBuf>,
    ) -> Result<Self, RenderError> {
        Ok(Self {
            backend: Backend::new(target, size, saved)?,
            loaded: None,
        })
    }

    /// Replaces the engine's own shader called `name` with `code`, for maps
    /// loaded from now on.
    pub fn set_built_in_shader(&mut self, name: &str, code: &str) {
        self.backend.set_built_in_shader(name, code);
    }

    /// What's gone wrong with shaders since this was last asked. Each shows
    /// green, or for the final pass isn't used.
    pub fn take_shader_problems(&mut self) -> Vec<String> {
        self.backend.take_shader_problems()
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
        let shown = ShownTiles::new(&mesh);
        self.loaded = Some(LoadedMap {
            map,
            mesh,
            visible: VisibleSet::default(),
            shown,
        });
        Ok(())
    }

    /// Gives `tile` its detailed light. Returns whether it did: with the atlas
    /// full it stays in its average colours, which is safe to leave.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub fn show_tile_light(&mut self, tile: u32, light: &TileLight) -> Result<bool, RenderError> {
        let Some(loaded) = &mut self.loaded else {
            return Ok(false);
        };
        let Some(image) = tile_image(&loaded.mesh, &loaded.map, tile, light) else {
            return Ok(false);
        };
        let Some(corner) = loaded.shown.place(tile, image.size) else {
            return Ok(false);
        };
        let patches = detail_patches(&loaded.map, &loaded.mesh, &image, corner);
        self.backend.write_tile_light(&image, corner, &patches)?;
        Ok(true)
    }

    /// Takes `tile`'s detail away again, back to its average colours.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub fn hide_tile_light(&mut self, tile: u32) -> Result<(), RenderError> {
        let Some(loaded) = &mut self.loaded else {
            return Ok(());
        };
        if loaded.shown.free(tile) {
            let patches = far_patches(&loaded.map, &loaded.mesh, tile);
            self.backend.write_vertices(&patches)?;
        }
        Ok(())
    }

    /// How many tiles are showing their detail.
    #[must_use]
    pub fn shown_tiles(&self) -> usize {
        self.loaded
            .as_ref()
            .map_or(0, |loaded| loaded.shown.count())
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

    /// Sets what the final pass does.
    pub fn set_post(&mut self, post: PostSettings) {
        self.backend.set_post(post);
    }

    /// Sets the part of the window a scene shows in, as left, top, width and
    /// height, or `None` for all of it. What's round it is the UI's.
    pub fn set_view_area(&mut self, area: Option<[u32; 4]>) {
        self.backend.set_view_area(area);
    }

    /// How many lights' shadows the last frame redrew.
    #[must_use]
    pub fn shadow_redraws(&self) -> usize {
        self.backend.shadow_redraws()
    }

    /// Sets how many samples a pixel scenes have: 1, 2, 4 or 8, as far as the
    /// device goes.
    ///
    /// # Errors
    ///
    /// Returns an error if what depends on it can't be rebuilt.
    pub fn set_antialiasing(&mut self, samples: u32) -> Result<(), RenderError> {
        self.backend.set_antialiasing(samples)
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
