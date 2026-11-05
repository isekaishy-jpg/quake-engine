//! The Vulkan side of the renderer. Only this file knows `vulkane`.

mod pipelines;
mod post;
mod record;
mod shadow;
mod upload;

use std::sync::Arc;

use engine_core::canvas::Quad;
use engine_core::math::Vec3;
use engine_core::window::NativeWindow;
use engine_map::{AliasModel, Map, Sprite};
use pipelines::make_pass;
use upload::{depth_image, device_image, host_buffer};
use vulkane::safe::{
    AccessFlags, ApiVersion, Buffer, BufferImageCopy, BufferUsage, ClearValue, CommandBuffer,
    CommandPool, CompareOp, DescriptorPool, DescriptorSet, DescriptorSetLayout,
    DescriptorSetLayoutBinding, DescriptorType, Device, DeviceCreateInfo, DeviceExtensions,
    DeviceFeatures, DeviceMemory, Fence, Format, Framebuffer, GraphicsPipeline, Image,
    Image2dCreateInfo, ImageLayout, ImageUsage, ImageView, Instance, InstanceCreateInfo,
    InstanceExtensions, PhysicalDevice, PipelineLayout, PipelineStage, PresentMode,
    PushConstantRange, Queue, QueueCreateInfo, QueueFlags, RenderPass, Sampler, SamplerAddressMode,
    SamplerCreateInfo, SamplerFilter, SamplerMipmapMode, Semaphore, ShaderStageFlags,
    SignalSemaphore, Surface, Swapchain, SwapchainCreateInfo, WaitSemaphore,
};

use crate::alias::ALIAS_PUSH_SIZE;
use crate::canvas::{QuadBatch, UiFrame};
use crate::error::RenderError;
use crate::live::LIVE_BYTES;
use crate::renderer::{Capture, Drawables, PostSettings, Target, TextureFilter, View};
use crate::sprite::{MAX_LIGHT_DRAWS, MAX_PARTICLE_DRAWS, PARTICLE_BYTES, SPRITE_PUSH_SIZE};
use crate::visible::VisibleSet;
use crate::world::{BatchKind, WorldMesh};

/// The colour format of every target. UNORM keeps the original's sums.
const COLOR_FORMAT: Format = Format::B8G8R8A8_UNORM;

/// The depth format.
const DEPTH_FORMAT: Format = Format::D32_SFLOAT;

/// The colour behind everything, seen with no map loaded.
const CLEAR_COLOR: [f32; 4] = [0.02, 0.02, 0.03, 1.0];

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
const EXTRA_FLOATS: usize = 4 * 16 + 4 + 256 * 4 + 16 * 4 * 2 + 4 + MAX_POOLS * 12 + 4 + 8;

/// How many pools can show in a frame.
const MAX_POOLS: usize = 8;

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

/// The world shader.
const WORLD_SHADER: &str = concat!(
    include_str!("../shaders/frame.wgsl"),
    include_str!("../shaders/world.wgsl"),
    include_str!("../shaders/material.wgsl"),
    include_str!("../shaders/sky.wgsl"),
);

/// The overlay shader.
const ALIAS_SHADER: &str = concat!(
    include_str!("../shaders/frame.wgsl"),
    include_str!("../shaders/alias.wgsl"),
);

/// The shader of sprites and particles.
const SPRITE_SHADER: &str = include_str!("../shaders/sprite.wgsl");

/// The final pass's shader.
const POST_SHADER: &str = concat!(
    include_str!("../shaders/frame.wgsl"),
    include_str!("../shaders/post.wgsl"),
);

/// The canvas's shader.
const CANVAS_SHADER: &str = include_str!("../shaders/canvas.wgsl");

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

/// A map on the GPU.
struct GpuMap {
    /// All vertices.
    vertices: GpuBuffer,
    /// The visible indices, rewritten when the view changes leaf.
    indices: GpuBuffer,
    /// Each texture's descriptor set, by map texture index.
    texture_sets: Vec<Option<DescriptorSet>>,
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
    /// Kept alive for the set.
    _atlas: GpuImage,
    /// Kept alive for the set.
    _direction_atlas: GpuImage,
    /// Kept alive for the frame set.
    _paint_atlas: GpuImage,
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
    vertices: GpuBuffer,
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
    /// Every pose's corners.
    poses: GpuBuffer,
    /// Each corner's place on the skin.
    uvs: GpuBuffer,
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
    /// Sprites'.
    sprite: PipelineLayout,
    /// Particles'.
    particle: PipelineLayout,
}

/// The pipelines.
struct Pipelines {
    /// Lit surfaces.
    solid: GraphicsPipeline,
    /// Lit surfaces with holes.
    cutout: GraphicsPipeline,
    /// Liquids.
    liquid: GraphicsPipeline,
    /// Masked materials.
    masked: GraphicsPipeline,
    /// Sky.
    sky: GraphicsPipeline,
    /// The sky sphere, drawn behind everything.
    sky_sphere: GraphicsPipeline,
    /// Faces showing the sky sphere.
    sky_window: GraphicsPipeline,
    /// Opaque and masked materials.
    material: GraphicsPipeline,
    /// See-through materials.
    blended: GraphicsPipeline,
    /// Alias models.
    alias: GraphicsPipeline,
    /// Outlines of models, as a pushed-out hull.
    alias_hull: GraphicsPipeline,
    /// Sprites.
    sprite: GraphicsPipeline,
    /// Particles.
    particle: GraphicsPipeline,
}

impl Pipelines {
    /// The pipeline for `kind`.
    fn get(&self, kind: BatchKind) -> &GraphicsPipeline {
        match kind {
            BatchKind::Solid => &self.solid,
            BatchKind::Cutout => &self.cutout,
            BatchKind::Liquid => &self.liquid,
            BatchKind::Sky => &self.sky,
            BatchKind::Material => &self.material,
            BatchKind::Masked => &self.masked,
            BatchKind::Blended => &self.blended,
        }
    }
}

/// Everything Vulkan. Fields drop in order, so each comes before what it
/// depends on.
pub(crate) struct Backend {
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
    particles: GpuBuffer,
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
    /// The final pass's settings.
    post: PostSettings,
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
    commands: Option<CommandBuffer>,
    /// Commands for a frame.
    command_pool: CommandPool,
    /// Signals when an image is ready to draw to.
    available: Semaphore,
    /// Whether a frame is being drawn.
    in_flight: bool,
    /// What says the frame in flight has finished.
    frame_done: Fence,
    /// The present mode.
    present_mode: PresentMode,
    /// The present mode when frames don't wait for the display.
    free_present_mode: PresentMode,
    /// The queue.
    queue: Queue,
    /// The queue's family.
    queue_family: u32,
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
    for physical in devices {
        for (family, properties) in physical.queue_family_properties().iter().enumerate() {
            let family = u32::try_from(family).unwrap_or(u32::MAX);
            if properties.queue_flags().contains(QueueFlags::GRAPHICS)
                && surface.is_none_or(|surface| surface.supports_present(&physical, family))
            {
                return Ok((physical, family));
            }
        }
    }
    Err(RenderError::no_device())
}

impl Backend {
    /// Sets up the device, passes and pipelines.
    pub(crate) fn new(target: Target, size: (u32, u32)) -> Result<Self, RenderError> {
        let window = match target {
            Target::Window(window) => Some(window),
            Target::Offscreen => None,
        };
        let extensions = InstanceExtensions::new()
            .khr_surface()
            .enable_raw(vulkane::raw::bindings::KHR_WIN32_SURFACE_EXTENSION_NAME);
        let instance = Instance::new(InstanceCreateInfo {
            application_name: Some("Quake Engine"),
            api_version: ApiVersion::V1_1,
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
        let features = DeviceFeatures::new().with_sampler_anisotropy();
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
            alias: textured_layout(ALIAS_PUSH_SIZE)?,
            sprite: textured_layout(SPRITE_PUSH_SIZE)?,
            particle: PipelineLayout::new(&device, &[&frame_layout])?,
        };
        let scene_pass = post::make_scene_pass(&device, 1)?;
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
        let uniforms = host_buffer(&device, &physical, FRAME_SIZE, BufferUsage::UNIFORM_BUFFER)?;
        let live_uniforms =
            host_buffer(&device, &physical, LIVE_BYTES, BufferUsage::UNIFORM_BUFFER)?;
        let shadows = shadow::Shadows::new(&device, &physical)?;
        let shadow_sampler = Sampler::new(
            &device,
            SamplerCreateInfo {
                compare_op: Some(CompareOp::LESS_OR_EQUAL),
                ..SamplerCreateInfo::default()
            },
        )?;
        let particles = host_buffer(
            &device,
            &physical,
            MAX_PARTICLE_DRAWS * PARTICLE_BYTES,
            BufferUsage::VERTEX_BUFFER,
        )?;
        let command_pool = CommandPool::new(&device, queue_family)?;
        let frame_done = Fence::new(&device)?;
        let available = Semaphore::binary(&device)?;
        let post_pipeline = Self::make_post_pipeline(&device, &post_layout, &present_pass, 1)?;
        let mut backend = Self {
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
            shadows,
            shadow_sampler,
            scene_target: None,
            post_pipeline,
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
            commands: None,
            available,
            in_flight: false,
            frame_done,
            present_mode,
            free_present_mode: present_mode,
            queue,
            queue_family,
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

    /// Waits for the frame in flight.
    fn wait_for_frame(&mut self) -> Result<(), RenderError> {
        if self.in_flight {
            self.frame_done.wait(u64::MAX)?;
            self.frame_done.reset()?;
            self.in_flight = false;
        }
        self.commands = None;
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
                let (x, y) = (x.min(extent.0 - 1), y.min(extent.1 - 1));
                ((x, y), (width.min(extent.0 - x), height.min(extent.1 - y)))
            }
            _ => ((0, 0), extent),
        }
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
    fn record_scene(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        scene: Option<&Scene<'_>>,
        view: &View,
        size: (u32, u32),
    ) {
        let Some(target) = &self.scene_target else {
            return;
        };
        let ranges = scene.map_or_else(Vec::new, |scene| self.shadow_ranges(scene));
        self.shadows.record(rec, self.map.as_ref(), &ranges);
        // The scene reads what shadows wrote.
        rec.memory_barrier(
            PipelineStage::ALL_GRAPHICS,
            PipelineStage::FRAGMENT_SHADER,
            AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            AccessFlags::SHADER_READ,
        );
        rec.begin_render_pass_ext(
            &self.scene_pass,
            &target.framebuffer,
            &[
                ClearValue::Color(CLEAR_COLOR),
                ClearValue::DepthStencil {
                    depth: 1.0,
                    stencil: 0,
                },
            ],
        );
        if let (Some(scene), Some(recorder)) = (scene, self.recorder()) {
            recorder.record(rec, scene, view, size);
        }
        rec.end_render_pass();
        // The final pass reads what this wrote.
        rec.memory_barrier(
            PipelineStage::ALL_GRAPHICS,
            PipelineStage::FRAGMENT_SHADER,
            AccessFlags::COLOR_ATTACHMENT_WRITE | AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
            AccessFlags::SHADER_READ,
        );
    }

    /// What a shadow map draws: the world's opaque faces, then moving models',
    /// as first indices, counts and offsets.
    fn shadow_ranges(&self, scene: &Scene<'_>) -> Vec<(u32, u32, Vec3)> {
        let Some(map) = &self.map else {
            return Vec::new();
        };
        let mut ranges = vec![(0, map.shadow_world_count, Vec3::ZERO)];
        for instance in scene.drawables.instances.iter().filter(|i| i.visible) {
            let found = scene.mesh.model_ranges.get(instance.model as usize);
            for range in found.into_iter().flatten() {
                let blended = scene.mesh.batches.get(range.batch as usize);
                if blended.is_some_and(|batch| batch.kind == BatchKind::Blended) {
                    continue;
                }
                let first = map.shadow_world_count + range.first;
                ranges.push((first, range.count, instance.origin));
            }
        }
        ranges
    }

    /// Records the final pass.
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
        rec.draw(3, 1, 0, 0);
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
        self.wait_for_frame()?;
        if self.resized || self.window.is_none() {
            self.resized = false;
            self.rebuild_window()?;
        }
        let Some(window) = &self.window else {
            return Ok(());
        };
        let extent = window.extent;
        let Ok(image) = window
            .swapchain
            .acquire_next_image(u64::MAX, Some(&self.available), None)
        else {
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
                &self.available,
                PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            )],
            &[SignalSemaphore::binary(&window.finished[at])],
            Some(&self.frame_done),
        )?;
        let presented = window
            .swapchain
            .present(&self.queue, image, &[&window.finished[at]]);
        self.commands = Some(commands);
        self.in_flight = true;
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
        #[expect(
            clippy::cast_precision_loss,
            reason = "capture sizes are far below 2^24"
        )]
        let aspect = width as f32 / height as f32;
        self.upload_frame(view, scene, aspect)?;
        self.upload_canvas(quads, (width, height))?;
        self.ensure_scene_target((width, height))?;
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
                self.record_scene(rec, scene, view, (width, height));
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
                self.record_post(rec, (0, 0), (width, height));
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
    }
}
