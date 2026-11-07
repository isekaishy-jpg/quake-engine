//! The Vulkan side of the renderer. Only this file knows `vulkane`.

mod clock;
mod deform;
mod field;
mod pipeline_cache;
mod pipelines;
mod post;
mod record;
mod shadow;
mod sim;
mod smaa;
mod terrain;
mod upload;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use engine_core::canvas::Quad;
use engine_core::math::Vec3;
use engine_core::window::NativeWindow;
use engine_map::{AliasModel, Map, ShaderCode, SharedPart, Sprite};
use engine_shaders::BUILT_IN;
use pipelines::make_pass;
use upload::{depth_image, device_image, host_buffer, uniform_buffer};
use vulkane::safe::{
    AccessFlags, ApiVersion, Buffer, BufferCopy, BufferImageCopy, BufferUsage, ClearValue,
    CommandBuffer, CommandPool, CompareOp, DescriptorPool, DescriptorSet, DescriptorSetLayout,
    DescriptorSetLayoutBinding, DescriptorType, Device, DeviceCreateInfo, DeviceExtensions,
    DeviceFeatures, DeviceMemory, Fence, Format, Framebuffer, GraphicsPipeline, Image,
    Image2dCreateInfo, ImageLayout, ImageUsage, ImageView, Instance, InstanceCreateInfo,
    InstanceExtensions, PhysicalDevice, PipelineLayout, PipelineStage, PresentMode,
    PushConstantRange, Queue, QueueCreateInfo, QueueFlags, RenderPass, Sampler, SamplerAddressMode,
    SamplerCreateInfo, SamplerFilter, SamplerMipmapMode, Semaphore, ShaderStageFlags,
    SignalSemaphore, Surface, Swapchain, SwapchainCreateInfo, WaitSemaphore,
};

pub use self::clock::GpuTimes;
use self::clock::{GpuClock, Span};
pub use self::deform::half_bits;
use self::deform::{Deform, GpuMoving};
use self::field::{FieldLayout, GpuField};
use self::sim::{GpuSims, SIM_PUSH, SimLayout};
use self::terrain::GpuTerrain;
use crate::canvas::{QuadBatch, UiFrame};
use crate::error::{RenderError, vulkan_is_new_enough};
use crate::instances::InstanceBatch;
use crate::live::LIVE_BYTES;
use crate::renderer::{Capture, Drawables, PostSettings, Target, TextureFilter, View};
use crate::sprite::{
    GPU_PARTICLE_PUSH, MAX_LIGHT_DRAWS, MAX_PARTICLE_DRAWS, PARTICLE_BYTES, SPRITE_PUSH_SIZE,
};
use crate::visible::VisibleSet;
use crate::world::{BatchKind, WorldMesh};

/// The colour format of every target. UNORM keeps the original's sums.
const COLOR_FORMAT: Format = Format::B8G8R8A8_UNORM;

/// What a scene is drawn in: linear light, with room for more than a screen shows.
const SCENE_FORMAT: Format = Format::R16G16B16A16_SFLOAT;

/// The depth format.
const DEPTH_FORMAT: Format = Format::D32_SFLOAT;

/// The colour behind everything, seen with no map loaded.
const CLEAR_COLOR: [f32; 4] = [0.02, 0.02, 0.03, 1.0];

/// The same in a scene, which holds linear light.
const SCENE_CLEAR_COLOR: [f32; 4] = [0.001_548, 0.001_548, 0.002_322, 1.0];

/// The `r_aa` setting that's SMAA.
const SMAA_SETTING: u8 = 2;

/// The size of the push constant holding a model's position.
const MODEL_OFFSET_SIZE: u32 = 16;

/// The `VK_INDEX_TYPE_UINT32` value.
const INDEX_TYPE_U32: u32 = 1;

/// Bytes in the frame uniforms: a matrix, two vectors, 64 styles, and each
/// light's vector and least light.
const FRAME_SIZE: usize =
    64 + 16 + 16 + 256 + MAX_LIGHT_DRAWS * 20 + SKY_BYTES + POST_BYTES + EXTRA_BYTES;

/// Bytes of stencil and capsule values in the frame uniforms.
const EXTRA_BYTES: usize = EXTRA_FLOATS * 4;

/// Floats of stencil and capsule values in the frame uniforms.
const EXTRA_FLOATS: usize = 4 * 16
    + 4
    + 256 * 4
    + 16 * 4 * 2
    + 4
    + MAX_POOLS * 16
    + 4
    + 8
    + 4
    + 4
    + MAX_FOG_VOLUMES * 16
    + 4
    + FOG_LIGHTS * 8;

/// How many pools can show in a frame.
const MAX_POOLS: usize = 8;

/// How many fog volumes a frame looks at, the nearest of them.
const MAX_FOG_VOLUMES: usize = 8;

/// How many moving lights fog takes.
const FOG_LIGHTS: usize = 4;

/// Bytes of final pass values in the frame uniforms.
const POST_BYTES: usize = 128;

/// Bytes of sky values in the frame uniforms.
const SKY_BYTES: usize = 112;

/// Texture frames per second, from `R_TextureAnimation`.
const ANIMATION_RATE: f32 = 5.0;

/// The most windows' worth of swapchain images expected.
const MIN_SWAPCHAIN_IMAGES: u32 = 2;

/// How much sampling sharpens surfaces seen at an angle.
const MATERIAL_ANISOTROPY: f32 = 8.0;

/// The smallest mip level textures reach, as a BSP's four levels do.
const TEXTURE_MAX_LOD: f32 = 3.0;

/// The smallest mip level materials reach.
const MATERIAL_MAX_LOD: f32 = 16.0;

/// What [`Backend`] needs to draw a loaded map.
pub(crate) struct Scene<'a> {
    /// The map.
    pub(crate) map: &'a Map,
    /// Its mesh.
    pub(crate) mesh: &'a WorldMesh,
    /// What's visible.
    pub(crate) visible: &'a VisibleSet,
    /// Everything else in the frame.
    pub(crate) drawables: Drawables<'a>,
    /// Whether `visible` changed since it was last uploaded.
    pub(crate) changed: bool,
}

/// A mesh on the GPU, kept once however many times it's drawn.
struct GpuMesh {
    /// Its corners' places and normals.
    places: GpuBuffer,
    /// Its corners' UVs.
    uvs: GpuBuffer,
    /// Its triangles.
    indices: GpuBuffer,
    /// Its parts by material, for each level of detail.
    levels: Vec<Vec<SharedPart>>,
    /// Those of the finest that cast shadows.
    shadow_parts: Vec<SharedPart>,
}

/// A copy of a shared mesh, as a shadow map draws it.
struct ShadowInstance {
    /// Which mesh.
    mesh: u32,
    /// What places it, column by column.
    placed: [f32; 16],
    /// Its middle.
    center: Vec3,
    /// How far it reaches from there.
    radius: f32,
}

/// A buffer with its memory.
struct GpuBuffer {
    /// The buffer.
    buffer: Buffer,
    /// Its memory.
    memory: DeviceMemory,
}

/// An image with its memory and view.
struct GpuImage {
    /// The view.
    view: ImageView,
    /// The image.
    image: Image,
    /// Its memory, kept alive.
    _memory: DeviceMemory,
}

/// A colour and depth pair to draw into, with its framebuffer.
struct Attachments {
    /// The framebuffer.
    framebuffer: Framebuffer,
    /// The depth image, kept alive.
    _depth: GpuImage,
}

/// What particles show, as one array.
struct ParticlePictures {
    /// The array.
    _image: GpuImage,
    /// What reads it.
    set: DescriptorSet,
    /// Kept alive for the set.
    _pool: DescriptorPool,
}

/// The window's swapchain and its per-image state.
struct WindowTarget {
    /// The swapchain.
    swapchain: Swapchain,
    /// One view per image, kept alive for the framebuffers.
    _views: Vec<ImageView>,
    /// One framebuffer per image.
    framebuffers: Vec<Framebuffer>,
    /// Signalled when an image finishes drawing, one per image.
    finished: Vec<Semaphore>,
    /// The shared depth image, kept alive.
    _depth: GpuImage,
    /// The size.
    extent: (u32, u32),
}

/// How many frames can be on their way at once. With one, the CPU and GPU take
/// turns, and neither is ever busy while the other is.
const FRAMES: usize = 2;

/// A tile's light and vertices, waiting to be written by a frame.
struct TileWrite {
    /// What's written.
    staging: GpuBuffer,
    /// The size of the tile's square in the atlas, and where it goes.
    picture: Option<(u32, [u32; 2])>,
    /// Where each atlas's part is in `staging`.
    offsets: Vec<u64>,
    /// Which vertices are written.
    copies: Vec<BufferCopy>,
}

/// What a frame in flight writes its uniforms into.
struct FrameValues {
    /// The frame's.
    frame: GpuBuffer,
    /// The moving lights'.
    live: GpuBuffer,
}

impl FrameValues {
    /// Somewhere for a frame's uniforms.
    fn new(device: &Device, physical: &PhysicalDevice) -> Result<Self, RenderError> {
        Ok(Self {
            frame: host_buffer(device, physical, FRAME_SIZE, BufferUsage::TRANSFER_SRC)?,
            live: host_buffer(device, physical, LIVE_BYTES, BufferUsage::TRANSFER_SRC)?,
        })
    }
}

/// A map on the GPU.
struct GpuMap {
    /// All vertices.
    vertices: GpuBuffer,
    /// Meshes that placements share.
    shared_meshes: Vec<GpuMesh>,
    /// The copies of them to draw, one buffer for each frame in flight.
    instances: [GpuBuffer; FRAMES],
    /// Where each mesh's copies are in those.
    instance_batches: [Vec<InstanceBatch>; FRAMES],
    /// The middle of each copy, in the order they're written.
    instance_centers: [Vec<Vec3>; FRAMES],
    /// All the copies, for shadow maps.
    shadow_instances: Vec<ShadowInstance>,
    /// Which level of detail each copy is at.
    instance_levels: Vec<usize>,
    /// Meshes that move, if it has any.
    moving: GpuMoving,
    /// Land.
    terrain: Option<GpuTerrain>,
    /// Pictures worked out a step at a time.
    fields: Vec<GpuField>,
    /// What effects' particles show.
    particle_pictures: ParticlePictures,
    /// The pipelines of the shaders effects have of their own.
    particle_shaders: Vec<ParticlePipelines>,
    /// What those were made from.
    particle_codes: Vec<Arc<ShaderCode>>,
    /// Particles compute runs, if its effects have any.
    sims: Option<GpuSims>,
    /// The visible indices, one for each frame in flight.
    indices: [GpuBuffer; FRAMES],
    /// Which of those are behind the view, which changes as it moves between
    /// leaves.
    indices_stale: [bool; FRAMES],
    /// Each texture's descriptor set, by map texture index.
    texture_sets: Vec<Option<DescriptorSet>>,
    /// Each texture's place in `Pipelines::surfaces`.
    texture_shaders: Vec<usize>,
    /// The cloud image's set, if the map has a sky sphere.
    sky_set: Option<DescriptorSet>,
    /// Whether each texture's set is a material's.
    material_sets: Vec<bool>,
    /// Every material's parameters, kept alive for the sets.
    _material_params: Option<GpuBuffer>,
    /// Kept alive for the sets.
    _textures: Vec<GpuImage>,
    /// Kept alive for the sets.
    _pool: DescriptorPool,
    /// The lightmap set.
    frame_set: DescriptorSet,
    /// Every opaque face, for shadow maps.
    shadow_indices: GpuBuffer,
    /// How many of those are the world's.
    shadow_world_count: u32,
    /// The world's, a part at a time.
    shadow_parts: Vec<shadow::ShadowRange>,
    /// The light atlas.
    atlas: GpuImage,
    /// Where the atlas's light comes from.
    direction_atlas: GpuImage,
    /// How much of each layer the atlas's samples paint.
    paint_atlas: GpuImage,
}

/// A flat picture on the GPU.
struct GpuPicture {
    /// Its set.
    set: DescriptorSet,
    /// Kept alive for the set.
    _pool: DescriptorPool,
    /// Kept alive for the set.
    _image: GpuImage,
}

/// What's drawn over the view.
struct CanvasPass {
    /// The pipeline.
    pipeline: GraphicsPipeline,
    /// Its layout.
    layout: PipelineLayout,
    /// The layout of a picture's set.
    set_layout: DescriptorSetLayout,
    /// How pictures are read.
    sampler: Sampler,
    /// Every picture, where the first is solid white.
    pictures: Vec<GpuPicture>,
    /// The vertices, rewritten every frame.
    vertices: [GpuBuffer; FRAMES],
    /// Which of them this frame drew into.
    slot: usize,
    /// What to draw this frame.
    batches: Vec<QuadBatch>,
    /// A UI to draw over that.
    ui: UiFrame,
    /// Where the UI's vertices start.
    ui_first: u32,
}

/// A model on the GPU.
struct GpuAlias {
    /// The model, for its scale and offset.
    model: Arc<AliasModel>,
    /// How many corners it has, shared where triangles share them.
    corners: u32,
    /// Every pose's corners, kept alive for `pose_set`.
    _poses: GpuBuffer,
    /// What the compute pass reads them through.
    pose_set: DescriptorSet,
    /// Kept alive for `pose_set`.
    _pose_pool: DescriptorPool,
    /// Its first pose, as a mesh's corners, for when there's no room to move them.
    rest: GpuBuffer,
    /// Each corner's place on the skin.
    uvs: GpuBuffer,
    /// Its triangles.
    indices: GpuBuffer,
    /// How many indices that is.
    index_count: u32,
    /// A descriptor set for each skin.
    skin_sets: Vec<DescriptorSet>,
    /// Kept alive for the sets.
    _skins: Vec<GpuImage>,
    /// Kept alive for the sets.
    _pool: DescriptorPool,
}

/// A sprite on the GPU.
struct GpuSprite {
    /// The sprite, for its pictures' sizes.
    sprite: Arc<Sprite>,
    /// A descriptor set for each picture.
    sets: Vec<DescriptorSet>,
    /// Kept alive for the sets.
    _images: Vec<GpuImage>,
    /// Kept alive for the sets.
    _pool: DescriptorPool,
}

/// The layouts of the pipelines.
struct Layouts {
    /// The world's.
    world: PipelineLayout,
    /// Materials'.
    material: PipelineLayout,
    /// Models'.
    alias: PipelineLayout,
    /// Land: what a material has, and its heights.
    terrain: PipelineLayout,
    /// Sprites'.
    sprite: PipelineLayout,
    /// Particles'.
    particle: PipelineLayout,
    /// Those the GPU works out.
    gpu_particle: PipelineLayout,
    /// Particles compute runs.
    sim_particle: PipelineLayout,
}

/// What draws surfaces with one shader.
struct SurfacePipelines {
    /// The shader's code, by which it's found again.
    code: String,
    /// Whether it can follow the depth pass.
    after_depth_safe: bool,
    /// What's opaque.
    opaque: GraphicsPipeline,
    /// What's opaque, with its depth already drawn.
    after_depth: GraphicsPipeline,
    /// What has holes.
    masked: GraphicsPipeline,
    /// What's see-through.
    blended: GraphicsPipeline,
    /// Copies of a shared mesh that are opaque.
    mesh_opaque: GraphicsPipeline,
    /// Copies of a shared mesh that are opaque, with their depth already drawn.
    mesh_after_depth: GraphicsPipeline,
    /// Copies of a shared mesh with holes.
    mesh_masked: GraphicsPipeline,
    /// Land.
    land: GraphicsPipeline,
    /// Land, with its depth already drawn.
    land_after_depth: GraphicsPipeline,
    /// Copies of a shared mesh that are see-through.
    mesh_blended: GraphicsPipeline,
}

impl SurfacePipelines {
    /// The pipeline for copies of a shared mesh, of `kind`.
    fn mesh(&self, kind: BatchKind) -> &GraphicsPipeline {
        match kind {
            BatchKind::Sky | BatchKind::Material => &self.mesh_opaque,
            BatchKind::Masked => &self.mesh_masked,
            BatchKind::Blended => &self.mesh_blended,
        }
    }

    /// The pipeline for `kind`.
    fn get(&self, kind: BatchKind) -> &GraphicsPipeline {
        match kind {
            BatchKind::Sky | BatchKind::Material => &self.opaque,
            BatchKind::Masked => &self.masked,
            BatchKind::Blended => &self.blended,
        }
    }
}

/// The pipelines.
struct Pipelines {
    /// Depth alone, ahead of what's expensive to shade.
    depth_only: GraphicsPipeline,
    /// Surfaces with nothing unusual about them, which is most.
    plain_after_depth: GraphicsPipeline,
    /// The sky sphere, drawn behind everything.
    sky_sphere: GraphicsPipeline,
    /// Faces showing the sky sphere.
    sky_window: GraphicsPipeline,
    /// Each shader in use, the standard one first.
    surfaces: Vec<SurfacePipelines>,
    /// The outline of what moves.
    mesh_hull: GraphicsPipeline,
    /// The depth of meshes.
    mesh_depth: GraphicsPipeline,
    /// The depth of land.
    land_depth: GraphicsPipeline,
    /// The fixture's models.
    alias: GraphicsPipeline,
    /// Sprites.
    sprite: GraphicsPipeline,
}

/// What draws particles.
struct ParticlePipelines {
    /// Those in a list.
    listed: GraphicsPipeline,
    /// Those the GPU works out.
    worked_out: GraphicsPipeline,
    /// Those compute runs.
    simulated: GraphicsPipeline,
}

/// Everything Vulkan. Fields drop in order, so each comes before what it
/// depends on.
pub(crate) struct Backend {
    /// Pipelines kept between runs.
    saved_pipelines: pipeline_cache::PipelineCacheHolder,
    /// What's gone wrong with shaders since this was last asked.
    shader_problems: Vec<String>,
    /// The code of the engine's own shaders, by name.
    built_in_shaders: HashMap<String, String>,
    /// The code of the final pass.
    post_code: String,
    /// The window target, once it has a size.
    window: Option<WindowTarget>,
    /// The wanted window size.
    wanted_size: (u32, u32),
    /// Whether the window was resized since the swapchain was built.
    resized: bool,
    /// The loaded map.
    map: Option<GpuMap>,
    /// What draws text and pictures, set up just after the rest.
    canvas: Option<CanvasPass>,
    /// The frame uniforms.
    uniforms: GpuBuffer,
    /// The frame set layout.
    frame_layout: DescriptorSetLayout,
    /// The texture set layout.
    texture_layout: DescriptorSetLayout,
    /// The layouts of the pipelines.
    layouts: Layouts,
    /// Every model uploaded.
    models: Vec<GpuAlias>,
    /// Every sprite uploaded.
    sprites: Vec<GpuSprite>,
    /// The particles to draw, rewritten each frame.
    particles: [GpuBuffer; FRAMES],
    /// The pipelines.
    pipelines: Pipelines,
    /// The pass ending ready to present.
    present_pass: RenderPass,
    /// The pass ending ready to read back.
    capture_pass: RenderPass,
    /// The pass a scene is drawn in.
    scene_pass: RenderPass,
    /// How many samples a pixel scenes have.
    samples: u32,
    /// The part of the window the scene shows in.
    view_area: Option<[u32; 4]>,
    /// The moving light uniforms.
    live_uniforms: GpuBuffer,
    /// What each frame in flight writes the uniforms into, for the GPU to copy
    /// when it reaches that frame.
    frame_values: [FrameValues; FRAMES],
    /// Which frame in flight this is.
    slot: usize,
    /// What's waiting for the next frame to write.
    tile_writes: RefCell<Vec<TileWrite>>,
    /// What each frame in flight is still reading from.
    written_tiles: [RefCell<Vec<TileWrite>>; FRAMES],
    /// Shadow maps for moving lights.
    shadows: shadow::Shadows,
    /// Compares against shadow maps.
    shadow_sampler: Sampler,
    /// Where the scene is drawn.
    scene_target: Option<post::SceneTarget>,
    /// The layout of the final pass's set.
    post_set_layout: DescriptorSetLayout,
    /// The final pass's layout.
    post_layout: PipelineLayout,
    /// The final pass.
    post_pipeline: GraphicsPipeline,
    /// The pass light in the air is drawn in.
    ray_pass: RenderPass,
    /// Light in the air.
    ray_pipeline: GraphicsPipeline,
    /// The final pass's settings.
    post: PostSettings,
    /// SMAA's passes.
    smaa: smaa::Smaa,
    /// A picture for unused stencils.
    blank_stencil: Option<GpuImage>,
    /// The stencils' pictures.
    stencil_images: Vec<GpuImage>,
    /// The probes, as slices stacked down an image.
    probe_image: Option<GpuImage>,
    /// The texture sampler.
    texture_sampler: Sampler,
    /// The material sampler.
    material_sampler: Sampler,
    /// What the samplers were last made for.
    filter: Option<TextureFilter>,
    /// The layout of a material's set.
    material_layout: DescriptorSetLayout,
    /// The lightmap sampler.
    light_sampler: Sampler,
    /// The buffer being recorded or in flight.
    commands: [Option<CommandBuffer>; FRAMES],
    /// Commands for a frame.
    command_pool: CommandPool,
    /// Signals when an image is ready to draw to, by frame.
    available: [Semaphore; FRAMES],
    /// Whether each frame is being drawn.
    in_flight: [bool; FRAMES],
    /// What says each frame has finished.
    frame_done: [Fence; FRAMES],
    /// The present mode.
    present_mode: PresentMode,
    /// The present mode when frames don't wait for the display.
    free_present_mode: PresentMode,
    /// The queue.
    queue: Queue,
    /// The queue's family.
    queue_family: u32,
    /// What moves meshes.
    deform: Deform,
    /// The layout of what land reads.
    terrain_set_layout: DescriptorSetLayout,
    /// The layout of the scene's depth, as particles read it.
    particle_depth_layout: DescriptorSetLayout,
    /// The layout of the pictures of particles.
    particle_picture_layout: DescriptorSetLayout,
    /// Where particles are drawn.
    particle_pass: RenderPass,
    /// The scene's pass, carrying on after particles behind what's see-through.
    scene_again_pass: RenderPass,
    /// The shader of each particle this frame, in the order they're drawn.
    particle_order: Vec<u32>,
    /// How many of those are behind something see-through.
    particles_behind: usize,
    /// Particles with no shader of their own.
    particle_pipelines: ParticlePipelines,
    /// How pictures of particles are read.
    particle_sampler: Sampler,
    /// What compute particles' steps are bound by.
    sim_layout: SimLayout,
    /// What a step with no field reads.
    sim_blank: Option<GpuImage>,
    /// Whether this frame has fog to go through a step at a time.
    fog_marched: bool,
    /// What fields' steps are bound by.
    field_layout: FieldLayout,
    /// The time this frame's field steps are told.
    field_time: f32,
    /// Times what the GPU does.
    clock: GpuClock,
    /// Whether a moving mesh has had to draw at rest.
    moving_overflow: bool,
    /// The GPU's name.
    device_name: String,
    /// The surface, if drawing to a window. It drops after the swapchain.
    surface: Option<Arc<Surface>>,
    /// The physical device.
    physical: PhysicalDevice,
    /// The device.
    device: Device,
    /// The instance, last to drop.
    _instance: Instance,
}

/// Creates a Vulkan surface for `window`.
fn make_surface(instance: &Instance, window: NativeWindow) -> Result<Surface, RenderError> {
    let NativeWindow::Win32 { hwnd, hinstance } = window;
    #[expect(unsafe_code, reason = "Vulkan takes the window as raw handles")]
    // SAFETY: the handles come from a live window, and `engine_platform`'s
    // lease keeps it alive until after this renderer is dropped.
    let surface = unsafe {
        Surface::from_win32(
            instance,
            std::ptr::without_provenance_mut(hinstance.cast_unsigned()),
            std::ptr::without_provenance_mut(hwnd.cast_unsigned()),
        )
    };
    Ok(surface?)
}

/// Picks a device and queue family that can draw, and present if needed.
fn pick_device(
    instance: &Instance,
    surface: Option<&Surface>,
) -> Result<(PhysicalDevice, u32), RenderError> {
    let devices = instance.enumerate_physical_devices()?;
    let mut too_old = None;
    for physical in devices {
        let properties = physical.properties();
        let version = properties.api_version();
        let version = (version.major(), version.minor());
        if !vulkan_is_new_enough(version) {
            too_old = Some(RenderError::too_old(&properties.device_name(), version));
            continue;
        }
        for (family, properties) in physical.queue_family_properties().iter().enumerate() {
            let family = u32::try_from(family).unwrap_or(u32::MAX);
            // One queue does both: moving meshes are worked out just before they're drawn.
            if properties.queue_flags().contains(QueueFlags::GRAPHICS)
                && properties.queue_flags().contains(QueueFlags::COMPUTE)
                && surface.is_none_or(|surface| surface.supports_present(&physical, family))
            {
                return Ok((physical, family));
            }
        }
    }
    Err(too_old.unwrap_or_else(RenderError::no_device))
}

impl Backend {
    /// Sets up the device, passes and pipelines.
    pub(crate) fn new(
        target: Target,
        size: (u32, u32),
        saved_pipelines: Option<PathBuf>,
    ) -> Result<Self, RenderError> {
        let window = match target {
            Target::Window(window) => Some(window),
            Target::Offscreen => None,
        };
        let extensions = InstanceExtensions::new()
            .khr_surface()
            .enable_raw(vulkane::raw::bindings::KHR_WIN32_SURFACE_EXTENSION_NAME);
        let instance = Instance::new(InstanceCreateInfo {
            application_name: Some("Quake Engine"),
            api_version: ApiVersion::V1_3,
            enabled_extensions: window.as_ref().map(|_| &extensions),
            ..InstanceCreateInfo::default()
        })
        .map_err(|_no_loader| RenderError::no_device())?;
        let surface = window
            .map(|window| make_surface(&instance, window).map(Arc::new))
            .transpose()?;
        let (physical, queue_family) = pick_device(&instance, surface.as_deref())?;
        let device_name = physical.properties().device_name().to_owned();
        let device_extensions = DeviceExtensions::new().khr_swapchain();
        let features = DeviceFeatures::new()
            .with_sampler_anisotropy()
            .with_texture_compression_bc()
            .with_synchronization2();
        let device = physical.create_device(DeviceCreateInfo {
            queue_create_infos: &[QueueCreateInfo::single(queue_family)],
            enabled_extensions: surface.as_ref().map(|_| &device_extensions),
            enabled_features: Some(&features),
            ..DeviceCreateInfo::default()
        })?;
        let queue = device.get_queue(queue_family, 0);
        let present_mode = match &surface {
            Some(surface)
                if surface
                    .present_modes(&physical)?
                    .contains(&PresentMode::MAILBOX) =>
            {
                PresentMode::MAILBOX
            }
            _ => PresentMode::FIFO,
        };
        let deform = Deform::new(&device)?;
        let present_pass = make_pass(&device, ImageLayout::PRESENT_SRC_KHR)?;
        let capture_pass = make_pass(&device, ImageLayout::TRANSFER_SRC_OPTIMAL)?;

        let image_binding = |binding, descriptor_type| DescriptorSetLayoutBinding {
            binding,
            descriptor_type,
            descriptor_count: 1,
            stage_flags: ShaderStageFlags::FRAGMENT,
        };
        let frame_layout = DescriptorSetLayout::new(
            &device,
            &[
                DescriptorSetLayoutBinding {
                    binding: 0,
                    descriptor_type: DescriptorType::UNIFORM_BUFFER,
                    descriptor_count: 1,
                    stage_flags: ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
                },
                image_binding(1, DescriptorType::SAMPLED_IMAGE),
                image_binding(2, DescriptorType::SAMPLER),
                image_binding(3, DescriptorType::SAMPLED_IMAGE),
                image_binding(4, DescriptorType::UNIFORM_BUFFER),
                image_binding(5, DescriptorType::SAMPLED_IMAGE),
                image_binding(6, DescriptorType::SAMPLER),
                image_binding(7, DescriptorType::SAMPLED_IMAGE),
            ],
        )?;
        let texture_layout = DescriptorSetLayout::new(
            &device,
            &[
                image_binding(0, DescriptorType::SAMPLED_IMAGE),
                image_binding(1, DescriptorType::SAMPLER),
            ],
        )?;
        let material_layout = DescriptorSetLayout::new(
            &device,
            &[
                image_binding(0, DescriptorType::SAMPLED_IMAGE),
                image_binding(1, DescriptorType::SAMPLER),
                image_binding(2, DescriptorType::SAMPLED_IMAGE),
                image_binding(3, DescriptorType::SAMPLED_IMAGE),
                image_binding(4, DescriptorType::SAMPLED_IMAGE),
                image_binding(5, DescriptorType::SAMPLED_IMAGE),
                image_binding(6, DescriptorType::SAMPLED_IMAGE),
                image_binding(7, DescriptorType::SAMPLED_IMAGE),
                image_binding(8, DescriptorType::SAMPLED_IMAGE),
                image_binding(9, DescriptorType::UNIFORM_BUFFER),
                image_binding(10, DescriptorType::SAMPLED_IMAGE),
                image_binding(11, DescriptorType::SAMPLED_IMAGE),
                image_binding(12, DescriptorType::SAMPLED_IMAGE),
                image_binding(13, DescriptorType::SAMPLED_IMAGE),
            ],
        )?;
        let post_set_layout = DescriptorSetLayout::new(
            &device,
            &[
                DescriptorSetLayoutBinding {
                    binding: 0,
                    descriptor_type: DescriptorType::UNIFORM_BUFFER,
                    descriptor_count: 1,
                    stage_flags: ShaderStageFlags::FRAGMENT,
                },
                image_binding(1, DescriptorType::SAMPLED_IMAGE),
                image_binding(2, DescriptorType::SAMPLED_IMAGE),
                image_binding(3, DescriptorType::SAMPLER),
                image_binding(4, DescriptorType::SAMPLED_IMAGE),
                image_binding(5, DescriptorType::SAMPLED_IMAGE),
                image_binding(6, DescriptorType::SAMPLED_IMAGE),
                image_binding(7, DescriptorType::SAMPLED_IMAGE),
                image_binding(8, DescriptorType::SAMPLED_IMAGE),
                image_binding(9, DescriptorType::SAMPLED_IMAGE),
                image_binding(10, DescriptorType::SAMPLED_IMAGE),
            ],
        )?;
        let post_layout = PipelineLayout::new(&device, &[&post_set_layout])?;
        let textured_layout = |size| {
            PipelineLayout::with_push_constants(
                &device,
                &[&frame_layout, &texture_layout],
                &[PushConstantRange {
                    stage_flags: ShaderStageFlags::VERTEX,
                    offset: 0,
                    size,
                }],
            )
        };
        let land_binding = |binding| DescriptorSetLayoutBinding {
            binding,
            descriptor_type: DescriptorType::STORAGE_BUFFER,
            descriptor_count: 1,
            stage_flags: ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
        };
        let land_picture = |binding| DescriptorSetLayoutBinding {
            binding,
            descriptor_type: DescriptorType::SAMPLED_IMAGE,
            descriptor_count: 1,
            stage_flags: ShaderStageFlags::FRAGMENT,
        };
        let particle_depth_layout = DescriptorSetLayout::new(
            &device,
            &[DescriptorSetLayoutBinding {
                binding: 2,
                descriptor_type: DescriptorType::SAMPLED_IMAGE,
                descriptor_count: 1,
                stage_flags: ShaderStageFlags::FRAGMENT | ShaderStageFlags::COMPUTE,
            }],
        )?;
        let particle_picture_layout = DescriptorSetLayout::new(
            &device,
            &[
                DescriptorSetLayoutBinding {
                    binding: 0,
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: 1,
                    stage_flags: ShaderStageFlags::FRAGMENT,
                },
                DescriptorSetLayoutBinding {
                    binding: 1,
                    descriptor_type: DescriptorType::SAMPLER,
                    descriptor_count: 1,
                    stage_flags: ShaderStageFlags::FRAGMENT,
                },
            ],
        )?;
        let sim_layout = SimLayout::new(&device, &particle_depth_layout)?;
        let terrain_set_layout = DescriptorSetLayout::new(
            &device,
            &[
                land_binding(0),
                land_binding(1),
                land_binding(2),
                land_binding(3),
                land_binding(4),
                land_binding(5),
                land_picture(6),
                land_picture(7),
            ],
        )?;
        let layouts = Layouts {
            world: textured_layout(MODEL_OFFSET_SIZE)?,
            material: PipelineLayout::with_push_constants(
                &device,
                &[&frame_layout, &material_layout],
                &[PushConstantRange {
                    stage_flags: ShaderStageFlags::VERTEX,
                    offset: 0,
                    size: MODEL_OFFSET_SIZE,
                }],
            )?,
            alias: PipelineLayout::new(&device, &[&frame_layout, &texture_layout])?,
            terrain: PipelineLayout::with_push_constants(
                &device,
                &[&frame_layout, &material_layout, &terrain_set_layout],
                &[PushConstantRange {
                    stage_flags: ShaderStageFlags::VERTEX,
                    offset: 0,
                    size: MODEL_OFFSET_SIZE,
                }],
            )?,
            sprite: textured_layout(SPRITE_PUSH_SIZE)?,
            particle: PipelineLayout::new(
                &device,
                &[
                    &frame_layout,
                    &particle_depth_layout,
                    &particle_picture_layout,
                ],
            )?,
            sim_particle: PipelineLayout::with_push_constants(
                &device,
                &[
                    &frame_layout,
                    &particle_depth_layout,
                    &particle_picture_layout,
                    &sim_layout.set_layout,
                ],
                &[PushConstantRange {
                    stage_flags: ShaderStageFlags::VERTEX,
                    offset: 0,
                    size: SIM_PUSH,
                }],
            )?,
            gpu_particle: PipelineLayout::with_push_constants(
                &device,
                &[
                    &frame_layout,
                    &particle_depth_layout,
                    &particle_picture_layout,
                ],
                &[PushConstantRange {
                    stage_flags: ShaderStageFlags::VERTEX,
                    offset: 0,
                    size: GPU_PARTICLE_PUSH,
                }],
            )?,
        };
        let saved_pipelines = pipeline_cache::open(&device, &physical, saved_pipelines.as_deref());
        let scene_pass = post::make_scene_pass(&device, 1)?;
        let particle_pass = post::make_particle_pass(&device, 1)?;
        let scene_again_pass = post::make_scene_again_pass(&device, 1)?;
        let particle_pipelines =
            Self::make_particle_pipelines(&device, &layouts, &particle_pass, 1, "")?;
        let particle_sampler = Sampler::new_mipmapped(&device, SamplerCreateInfo::default(), 16.0)?;
        let pipelines = Self::make_pipelines(&device, &layouts, &scene_pass, 1)?;
        let texture_sampler = Sampler::new_mipmapped(
            &device,
            SamplerCreateInfo {
                mag_filter: SamplerFilter::NEAREST,
                min_filter: SamplerFilter::NEAREST,
                mipmap_mode: SamplerMipmapMode::LINEAR,
                address_mode_u: SamplerAddressMode::REPEAT,
                address_mode_v: SamplerAddressMode::REPEAT,
                address_mode_w: SamplerAddressMode::REPEAT,
                ..SamplerCreateInfo::default()
            },
            TEXTURE_MAX_LOD,
        )?;
        let material_sampler = Sampler::new_mipmapped(
            &device,
            SamplerCreateInfo {
                address_mode_u: SamplerAddressMode::REPEAT,
                address_mode_v: SamplerAddressMode::REPEAT,
                address_mode_w: SamplerAddressMode::REPEAT,
                anisotropy: Some(MATERIAL_ANISOTROPY),
                ..SamplerCreateInfo::default()
            },
            MATERIAL_MAX_LOD,
        )?;
        let light_sampler =
            Sampler::new_mipmapped(&device, SamplerCreateInfo::default(), MATERIAL_MAX_LOD)?;
        let uniforms = uniform_buffer(&device, &physical, FRAME_SIZE)?;
        let live_uniforms = uniform_buffer(&device, &physical, LIVE_BYTES)?;
        let frame_values = [
            FrameValues::new(&device, &physical)?,
            FrameValues::new(&device, &physical)?,
        ];
        let shadows = shadow::Shadows::new(&device, &physical, &terrain_set_layout)?;
        let field_layout = FieldLayout::new(&device)?;
        let shadow_sampler = Sampler::new(
            &device,
            SamplerCreateInfo {
                compare_op: Some(CompareOp::LESS_OR_EQUAL),
                ..SamplerCreateInfo::default()
            },
        )?;
        let particle_buffer = || {
            host_buffer(
                &device,
                &physical,
                MAX_PARTICLE_DRAWS * PARTICLE_BYTES,
                BufferUsage::VERTEX_BUFFER,
            )
        };
        let particles = [particle_buffer()?, particle_buffer()?];
        let command_pool = CommandPool::new(&device, queue_family)?;
        let clock = GpuClock::new(&device, &physical, queue_family);
        let frame_done = [Fence::new(&device)?, Fence::new(&device)?];
        let available = [Semaphore::binary(&device)?, Semaphore::binary(&device)?];
        let post_pipeline =
            Self::make_post_pipeline(&device, &post_layout, &present_pass, 1, "fs_post", "")?;
        let ray_pass = post::make_ray_pass(&device)?;
        let ray_pipeline =
            Self::make_post_pipeline(&device, &post_layout, &ray_pass, 1, "fs_rays", "")?;
        let smaa = smaa::Smaa::new(&device)?;
        let mut backend = Self {
            saved_pipelines,
            shader_problems: Vec::new(),
            built_in_shaders: BUILT_IN
                .iter()
                .map(|(name, code)| ((*name).to_owned(), (*code).to_owned()))
                .collect(),
            post_code: String::new(),
            surface,
            window: None,
            wanted_size: size,
            resized: false,
            map: None,
            canvas: None,
            uniforms,
            frame_layout,
            texture_layout,
            layouts,
            models: Vec::new(),
            sprites: Vec::new(),
            particles,
            pipelines,
            present_pass,
            capture_pass,
            scene_pass,
            samples: 1,
            view_area: None,
            live_uniforms,
            frame_values,
            slot: 0,
            tile_writes: RefCell::default(),
            written_tiles: [RefCell::default(), RefCell::default()],
            shadows,
            shadow_sampler,
            scene_target: None,
            post_pipeline,
            ray_pass,
            ray_pipeline,
            smaa,
            post_set_layout,
            post_layout,
            post: PostSettings::default(),
            blank_stencil: None,
            stencil_images: Vec::new(),
            probe_image: None,
            texture_sampler,
            material_sampler,
            filter: None,
            material_layout,
            light_sampler,
            command_pool,
            commands: [None, None],
            available,
            in_flight: [false; FRAMES],
            frame_done,
            present_mode,
            free_present_mode: present_mode,
            queue,
            queue_family,
            deform,
            clock,
            terrain_set_layout,
            particle_depth_layout,
            particle_picture_layout,
            particle_pass,
            scene_again_pass,
            particle_order: Vec::new(),
            particles_behind: 0,
            particle_pipelines,
            particle_sampler,
            sim_layout,
            sim_blank: None,
            fog_marched: false,
            field_layout,
            field_time: 0.0,
            moving_overflow: false,
            device_name,
            physical,
            device,
            _instance: instance,
        };
        backend.canvas = Some(backend.make_canvas()?);
        let _white = backend.add_picture(1, 1, &[255, 255, 255, 255])?;
        backend.rebuild_window()?;
        Ok(backend)
    }

    /// The size of what's being drawn to, in pixels.
    pub(crate) fn size(&self) -> (u32, u32) {
        self.window
            .as_ref()
            .map_or(self.wanted_size, |window| window.extent)
    }

    /// The GPU's name.
    pub(crate) fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Waits for frame `slot`, if it's in flight.
    fn wait_for_slot(&mut self, slot: usize) -> Result<(), RenderError> {
        if self.in_flight[slot] {
            self.frame_done[slot].wait(u64::MAX)?;
            self.frame_done[slot].reset()?;
            self.in_flight[slot] = false;
            self.clock.read(slot);
        }
        self.commands[slot] = None;
        self.written_tiles[slot].borrow_mut().clear();
        Ok(())
    }

    /// Waits for every frame in flight.
    fn wait_for_frame(&mut self) -> Result<(), RenderError> {
        for slot in 0..FRAMES {
            self.wait_for_slot(slot)?;
        }
        Ok(())
    }

    /// Makes the swapchain match the wanted size. A zero size leaves no
    /// swapchain, as when minimized.
    fn rebuild_window(&mut self) -> Result<(), RenderError> {
        let Some(surface) = self.surface.clone() else {
            return Ok(());
        };
        self.device.wait_idle()?;
        self.window = None;
        let capabilities = surface.capabilities(&self.physical)?;
        let (mut width, mut height) = capabilities.current_extent();
        if width == u32::MAX {
            (width, height) = self.wanted_size;
        }
        if width == 0 || height == 0 {
            return Ok(());
        }
        let swapchain = Swapchain::new(
            &self.device,
            &surface,
            SwapchainCreateInfo {
                format: COLOR_FORMAT,
                width,
                height,
                min_image_count: capabilities.min_image_count().max(MIN_SWAPCHAIN_IMAGES + 1),
                present_mode: self.present_mode,
                ..SwapchainCreateInfo::default()
            },
        )?;
        let depth = depth_image(&self.device, &self.physical, (width, height))?;
        let views = swapchain.image_views()?;
        let mut framebuffers = Vec::with_capacity(views.len());
        let mut finished = Vec::with_capacity(views.len());
        for view in &views {
            framebuffers.push(Framebuffer::new(
                &self.device,
                &self.present_pass,
                &[view, &depth.view],
                width,
                height,
            )?);
            finished.push(Semaphore::binary(&self.device)?);
        }
        self.window = Some(WindowTarget {
            swapchain,
            _views: views,
            framebuffers,
            finished,
            _depth: depth,
            extent: (width, height),
        });
        Ok(())
    }

    /// Chooses whether frames wait for the display.
    pub(crate) fn set_vsync(&mut self, vsync: bool) {
        let mode = if vsync {
            PresentMode::FIFO
        } else {
            self.free_present_mode
        };
        if mode != self.present_mode {
            self.present_mode = mode;
            self.resized = true;
        }
    }

    /// Replaces the engine's own shader called `name` with `code`.
    pub(crate) fn set_built_in_shader(&mut self, name: &str, code: &str) {
        self.built_in_shaders
            .insert(name.to_owned(), code.to_owned());
    }

    /// What's gone wrong with shaders since this was last asked.
    pub(crate) fn take_shader_problems(&mut self) -> Vec<String> {
        std::mem::take(&mut self.shader_problems)
    }

    /// Makes the samplers for `filter`, and points everything at them.
    pub(crate) fn set_filter(&mut self, filter: TextureFilter) -> Result<(), RenderError> {
        if self.filter == Some(filter) {
            return Ok(());
        }
        self.wait_for_frame()?;
        self.device.wait_idle()?;
        let smooth = if filter.smooth {
            SamplerFilter::LINEAR
        } else {
            SamplerFilter::NEAREST
        };
        let mipmap_mode = if filter.trilinear {
            SamplerMipmapMode::LINEAR
        } else {
            SamplerMipmapMode::NEAREST
        };
        let anisotropy = (filter.anisotropy > 1.0 && filter.smooth).then_some(filter.anisotropy);
        let material_anisotropy = (filter.anisotropy > 1.0).then_some(filter.anisotropy);
        let info = SamplerCreateInfo {
            mag_filter: smooth,
            min_filter: smooth,
            mipmap_mode,
            address_mode_u: SamplerAddressMode::REPEAT,
            address_mode_v: SamplerAddressMode::REPEAT,
            address_mode_w: SamplerAddressMode::REPEAT,
            anisotropy,
            ..SamplerCreateInfo::default()
        };
        let texture_sampler = Sampler::new_mipmapped(&self.device, info, TEXTURE_MAX_LOD)?;
        let material_sampler = Sampler::new_mipmapped(
            &self.device,
            SamplerCreateInfo {
                mag_filter: SamplerFilter::LINEAR,
                min_filter: SamplerFilter::LINEAR,
                mipmap_mode: SamplerMipmapMode::LINEAR,
                anisotropy: material_anisotropy,
                ..info
            },
            MATERIAL_MAX_LOD,
        )?;
        if let Some(map) = &self.map {
            for (set, is_material) in map.texture_sets.iter().zip(&map.material_sets) {
                if let Some(set) = set {
                    let sampler = if *is_material {
                        &material_sampler
                    } else {
                        &texture_sampler
                    };
                    set.write_sampler(1, sampler);
                }
            }
        }
        for model in &self.models {
            for set in &model.skin_sets {
                set.write_sampler(1, &texture_sampler);
            }
        }
        for sprite in &self.sprites {
            for set in &sprite.sets {
                set.write_sampler(1, &texture_sampler);
            }
        }
        self.texture_sampler = texture_sampler;
        self.material_sampler = material_sampler;
        self.filter = Some(filter);
        Ok(())
    }

    /// Sets the UI to draw over the next frames.
    pub(crate) fn set_ui(&mut self, ui: UiFrame) {
        if let Some(canvas) = &mut self.canvas {
            canvas.ui = ui;
        }
    }

    /// Sets the final pass's settings.
    pub(crate) fn set_post(&mut self, post: PostSettings) {
        self.post = post;
    }

    /// Sets the part of the window the scene shows in, or `None` for all of it.
    pub(crate) fn set_view_area(&mut self, area: Option<[u32; 4]>) {
        self.view_area = area;
    }

    /// Where in a window of `extent` the scene shows, as a corner and a size.
    fn view_area(&self, extent: (u32, u32)) -> ((u32, u32), (u32, u32)) {
        match self.view_area {
            Some([x, y, width, height]) if width > 0 && height > 0 => {
                let x = x.min(extent.0.saturating_sub(1));
                let y = y.min(extent.1.saturating_sub(1));
                let size = (
                    width.min(extent.0.saturating_sub(x)).max(1),
                    height.min(extent.1.saturating_sub(y)).max(1),
                );
                ((x, y), size)
            }
            _ => ((0, 0), extent),
        }
    }

    /// Starts or stops timing what the GPU does. Says if it can.
    pub(crate) fn time_gpu(&mut self, on: bool) -> bool {
        self.clock.set_on(on)
    }

    /// What the GPU's clock has seen since this was last asked.
    pub(crate) fn take_gpu_times(&mut self) -> GpuTimes {
        self.clock.take()
    }

    /// Whether a moving mesh has had to draw at rest since this was last asked.
    pub(crate) fn take_moving_overflow(&mut self) -> bool {
        std::mem::take(&mut self.moving_overflow)
    }

    /// How many lights' shadows the last frame redrew.
    pub(crate) fn shadow_redraws(&self) -> usize {
        self.shadows.redraws()
    }

    /// Sets how many samples a pixel scenes have, as far as the device goes.
    pub(crate) fn set_antialiasing(&mut self, samples: u32) -> Result<(), RenderError> {
        self.set_samples(samples)
    }

    /// Records the scene into its target.
    /// Starts a frame on the GPU. Another may still be drawing, and they share
    /// what's drawn to, so this waits for that much. Then it takes this frame's
    /// uniforms, which only the GPU writes where shaders read them.
    fn record_values(&self, rec: &mut vulkane::safe::CommandBufferRecording<'_>) {
        rec.memory_barrier(
            PipelineStage::ALL_COMMANDS,
            PipelineStage::ALL_COMMANDS,
            AccessFlags::MEMORY_WRITE,
            AccessFlags::MEMORY_READ | AccessFlags::MEMORY_WRITE,
        );
        self.record_tile_writes(rec);
        let values = &self.frame_values[self.slot];
        let copy = |size: usize| BufferCopy {
            src_offset: 0,
            dst_offset: 0,
            size: size as u64,
        };
        rec.copy_buffer(
            &values.frame.buffer,
            &self.uniforms.buffer,
            &[copy(FRAME_SIZE)],
        );
        rec.copy_buffer(
            &values.live.buffer,
            &self.live_uniforms.buffer,
            &[copy(LIVE_BYTES)],
        );
        rec.memory_barrier(
            PipelineStage::TRANSFER,
            PipelineStage::ALL_GRAPHICS,
            AccessFlags::TRANSFER_WRITE,
            AccessFlags::UNIFORM_READ,
        );
    }

    fn record_scene(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: Option<&Scene<'_>>,
        view: &View,
        size: (u32, u32),
    ) {
        self.clock.begin(rec, self.slot);
        self.record_values(rec);
        let Some(target) = &self.scene_target else {
            return;
        };
        self.clock.start(rec, self.slot, Span::Deform);
        self.record_deform(rec);
        self.record_fields(rec);
        self.record_sims(rec);
        self.clock.end(rec, self.slot, Span::Deform);
        let wanted = scene.filter(|_| self.shadows.redraws() > 0);
        let ranges = wanted.map_or_else(Vec::new, |scene| self.shadow_ranges(scene));
        let entities = wanted.map_or(&[][..], |scene| scene.drawables.entities);
        let places = self
            .map
            .as_ref()
            .map_or(&[][..], |map| map.moving.figure_places(self.slot));
        let figures = shadow::shadow_figures(&self.models, entities, places);
        self.clock.start(rec, self.slot, Span::Shadows);
        self.shadows
            .record(rec, self.map.as_ref(), &ranges, &figures, self.slot);
        self.clock.end(rec, self.slot, Span::Shadows);
        // The scene reads what shadows wrote.
        rec.memory_barrier(
            PipelineStage::ALL_GRAPHICS,
            PipelineStage::FRAGMENT_SHADER,
            AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            AccessFlags::SHADER_READ,
        );
        self.clock.start(rec, self.slot, Span::Scene);
        rec.begin_render_pass_ext(
            &self.scene_pass,
            &target.framebuffer,
            &[
                ClearValue::Color(SCENE_CLEAR_COLOR),
                ClearValue::DepthStencil {
                    depth: 1.0,
                    stencil: 0,
                },
            ],
        );
        if let (Some(scene), Some(recorder)) = (scene, self.recorder()) {
            recorder.record_solid(rec, scene, view, size);
            let behind = self.particles_behind;
            if behind > 0 {
                rec.end_render_pass();
                self.record_particle_pass(rec, &recorder, scene, target, (size, 0..behind));
                rec.begin_render_pass_ext(&self.scene_again_pass, &target.framebuffer, &[]);
            }
            recorder.record_see_through(rec, scene, view, size);
        }
        rec.end_render_pass();
        if let (Some(scene), Some(recorder)) = (scene, self.recorder())
            && recorder.has_particles(scene)
        {
            let rest = self.particles_behind..self.particle_order.len();
            self.record_particle_pass(rec, &recorder, scene, target, (size, rest));
        }
        target.drawn.set(true);
        self.clock.end(rec, self.slot, Span::Scene);
        // The final pass reads what this wrote.
        rec.memory_barrier(
            PipelineStage::ALL_GRAPHICS,
            PipelineStage::FRAGMENT_SHADER,
            AccessFlags::COLOR_ATTACHMENT_WRITE | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            AccessFlags::SHADER_READ,
        );
        let wanted = self.post.edge_smoothing >= SMAA_SETTING;
        self.smaa.record(rec, &target.smaa, target.size, wanted);
        if self.fog_marched || (self.post.rays != 0 && !self.stencil_images.is_empty()) {
            self.record_rays(rec, target);
        }
    }

    /// Records particles over what the scene has so far: they read its depth and add to its colour.
    fn record_particle_pass(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        recorder: &record::Recorder<'_>,
        scene: &Scene<'_>,
        target: &post::SceneTarget,
        (size, range): ((u32, u32), std::ops::Range<usize>),
    ) {
        rec.memory_barrier(
            PipelineStage::ALL_GRAPHICS,
            PipelineStage::ALL_GRAPHICS,
            AccessFlags::COLOR_ATTACHMENT_WRITE | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            AccessFlags::SHADER_READ | AccessFlags::COLOR_ATTACHMENT_WRITE,
        );
        rec.begin_render_pass_ext(&self.particle_pass, &target.particle_framebuffer, &[]);
        let last = range.end == self.particle_order.len();
        recorder.record_particles(rec, scene, size, &target.particle_set, range);
        if last {
            recorder.record_worked_out(rec, scene, &target.particle_set);
        }
        rec.end_render_pass();
    }

    /// What a shadow map draws: the world's opaque faces, then moving models',
    /// as first indices, counts and offsets.
    fn shadow_ranges(&self, scene: &Scene<'_>) -> Vec<shadow::ShadowRange> {
        let Some(map) = &self.map else {
            return Vec::new();
        };
        let mut ranges = map.shadow_parts.clone();
        for instance in scene.drawables.instances.iter().filter(|i| i.visible) {
            let found = scene.mesh.model_ranges.get(instance.model as usize);
            for range in found.into_iter().flatten() {
                let blended = scene.mesh.batches.get(range.batch as usize);
                if blended.is_some_and(|batch| batch.kind == BatchKind::Blended) {
                    continue;
                }
                let first = map.shadow_world_count + range.first;
                ranges.push(shadow::ShadowRange {
                    first,
                    count: range.count,
                    offset: instance.origin,
                    bounds: None,
                });
            }
        }
        ranges
    }

    /// Records the final pass.
    /// Draws light in the air, for the final pass to add.
    fn record_rays(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        target: &post::SceneTarget,
    ) {
        let size = target.rays.size;
        rec.begin_render_pass_ext(&self.ray_pass, &target.rays.framebuffer, &[]);
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        rec.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32);
        rec.set_scissor(0, 0, size.0, size.1);
        rec.bind_graphics_pipeline(&self.ray_pipeline);
        rec.bind_graphics_descriptor_sets(&self.post_layout, 0, &[&target.set]);
        self.clock.start(rec, self.slot, Span::Air);
        rec.draw(3, 1, 0, 0);
        self.clock.end(rec, self.slot, Span::Air);
        rec.end_render_pass();
        rec.memory_barrier(
            PipelineStage::ALL_GRAPHICS,
            PipelineStage::FRAGMENT_SHADER,
            AccessFlags::COLOR_ATTACHMENT_WRITE,
            AccessFlags::SHADER_READ,
        );
    }

    fn record_post(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        corner: (u32, u32),
        size: (u32, u32),
    ) {
        let Some(target) = &self.scene_target else {
            return;
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        rec.set_viewport(
            corner.0 as f32,
            corner.1 as f32,
            size.0 as f32,
            size.1 as f32,
        );
        rec.set_scissor(
            i32::try_from(corner.0).unwrap_or(0),
            i32::try_from(corner.1).unwrap_or(0),
            size.0,
            size.1,
        );
        rec.bind_graphics_pipeline(&self.post_pipeline);
        rec.bind_graphics_descriptor_sets(&self.post_layout, 0, &[&target.set]);
        self.clock.start(rec, self.slot, Span::Post);
        rec.draw(3, 1, 0, 0);
        self.clock.end(rec, self.slot, Span::Post);
    }

    /// Notes the window's new size.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.wanted_size = (width, height);
        self.resized = true;
    }

    /// Draws to the window.
    pub(crate) fn draw(
        &mut self,
        view: &View,
        scene: Option<&Scene<'_>>,
        quads: &[Quad],
    ) -> Result<(), RenderError> {
        self.slot = (self.slot + 1) % FRAMES;
        let slot = self.slot;
        self.wait_for_slot(slot)?;
        if self.resized || self.window.is_none() {
            self.resized = false;
            self.wait_for_frame()?;
            self.rebuild_window()?;
        }
        let Some(window) = &self.window else {
            return Ok(());
        };
        let extent = window.extent;
        let Ok(image) =
            window
                .swapchain
                .acquire_next_image(u64::MAX, Some(&self.available[slot]), None)
        else {
            self.wait_for_frame()?;
            return self.rebuild_window();
        };
        let (corner, shown) = self.view_area(extent);
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        let aspect = shown.0 as f32 / shown.1.max(1) as f32;
        self.upload_frame(view, scene, aspect)?;
        self.upload_canvas(quads, extent)?;
        self.ensure_scene_target(shown)?;
        let Some(window) = &self.window else {
            return Ok(());
        };
        let at = image as usize;
        let mut commands = self.command_pool.allocate_primary()?;
        {
            let mut rec = commands.begin()?;
            self.record_scene(&mut rec, scene, view, shown);
            rec.begin_render_pass_ext(
                &self.present_pass,
                &window.framebuffers[at],
                &[
                    ClearValue::Color(CLEAR_COLOR),
                    ClearValue::DepthStencil {
                        depth: 1.0,
                        stencil: 0,
                    },
                ],
            );
            self.record_post(&mut rec, corner, shown);
            Self::record_canvas(&mut rec, self.canvas.as_ref(), extent);
            rec.end_render_pass();
            rec.end()?;
        }
        self.queue.submit_with_sync(
            &[&commands],
            &[WaitSemaphore::binary(
                &self.available[slot],
                PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            )],
            &[SignalSemaphore::binary(&window.finished[at])],
            Some(&self.frame_done[slot]),
        )?;
        let presented = window
            .swapchain
            .present(&self.queue, image, &[&window.finished[at]]);
        self.commands[slot] = Some(commands);
        self.in_flight[slot] = true;
        if presented.is_err() {
            self.wait_for_frame()?;
            self.rebuild_window()?;
        }
        Ok(())
    }

    /// Draws off-screen and reads the pixels back.
    pub(crate) fn capture(
        &mut self,
        view: &View,
        scene: Option<&Scene<'_>>,
        quads: &[Quad],
        width: u32,
        height: u32,
    ) -> Result<Capture, RenderError> {
        self.wait_for_frame()?;
        self.device.wait_idle()?;
        let color = Image::new_2d(
            &self.device,
            Image2dCreateInfo {
                format: COLOR_FORMAT,
                width,
                height,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC,
            },
        )?;
        let color = device_image(&self.device, &self.physical, color, false)?;
        let depth = depth_image(&self.device, &self.physical, (width, height))?;
        let attachments = Attachments {
            framebuffer: Framebuffer::new(
                &self.device,
                &self.capture_pass,
                &[&color.view, &depth.view],
                width,
                height,
            )?,
            _depth: depth,
        };
        let size = (width as usize) * (height as usize) * 4;
        let mut readback = host_buffer(
            &self.device,
            &self.physical,
            size,
            BufferUsage::TRANSFER_DST,
        )?;
        // A picture of the window, so the scene's where the window has it.
        let (corner, shown) = self.view_area((width, height));
        #[expect(
            clippy::cast_precision_loss,
            reason = "capture sizes are far below 2^24"
        )]
        let aspect = shown.0 as f32 / shown.1.max(1) as f32;
        self.upload_frame(view, scene, aspect)?;
        self.upload_canvas(quads, (width, height))?;
        self.ensure_scene_target(shown)?;
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
                self.record_scene(rec, scene, view, shown);
                rec.begin_render_pass_ext(
                    &self.capture_pass,
                    &attachments.framebuffer,
                    &[
                        ClearValue::Color(CLEAR_COLOR),
                        ClearValue::DepthStencil {
                            depth: 1.0,
                            stencil: 0,
                        },
                    ],
                );
                self.record_post(rec, corner, shown);
                Self::record_canvas(rec, self.canvas.as_ref(), (width, height));
                rec.end_render_pass();
                rec.copy_image_to_buffer(
                    &color.image,
                    ImageLayout::TRANSFER_SRC_OPTIMAL,
                    &readback.buffer,
                    &[BufferImageCopy::full_2d(width, height)],
                );
                rec.memory_barrier(
                    PipelineStage::TRANSFER,
                    PipelineStage::HOST,
                    AccessFlags::TRANSFER_WRITE,
                    AccessFlags::HOST_READ,
                );
                Ok(())
            })?;
        self.clock.read(self.slot);
        let mut rgba = readback.memory.map()?.as_slice()[..size].to_vec();
        let (pixels, _remainder) = rgba.as_chunks_mut::<4>();
        for pixel in pixels {
            pixel.swap(0, 2);
        }
        Ok(Capture {
            width,
            height,
            rgba,
        })
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        let _ignored = self.device.wait_idle();
        self.saved_pipelines.save(&self.physical);
    }
}
