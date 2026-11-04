//! The Vulkan side of the renderer. Only this file knows `vulkane`.

use std::sync::Arc;

use engine_core::window::NativeWindow;
use engine_map::{Map, TextureKind};
use vulkane::safe::{
    AccessFlags, ApiVersion, AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, Buffer,
    BufferCopy, BufferCreateInfo, BufferImageCopy, BufferUsage, ClearValue, CommandBuffer,
    CommandPool, CullMode, DescriptorPool, DescriptorPoolSize, DescriptorSet, DescriptorSetLayout,
    DescriptorSetLayoutBinding, DescriptorType, Device, DeviceCreateInfo, DeviceExtensions,
    DeviceMemory, Fence, Format, Framebuffer, GraphicsPipeline, GraphicsPipelineBuilder,
    GraphicsShaderStage, Image, Image2dCreateInfo, ImageBarrier, ImageLayout, ImageUsage,
    ImageView, InputRate, Instance, InstanceCreateInfo, InstanceExtensions, MemoryPropertyFlags,
    PhysicalDevice, PipelineLayout, PipelineStage, PresentMode, Queue, QueueCreateInfo, QueueFlags,
    RenderPass, RenderPassCreateInfo, Sampler, SamplerAddressMode, SamplerCreateInfo,
    SamplerFilter, SamplerMipmapMode, Semaphore, ShaderModule, ShaderStageFlags, SignalSemaphore,
    Surface, Swapchain, SwapchainCreateInfo, VertexInputAttribute, VertexInputBinding,
    WaitSemaphore,
};

use crate::error::RenderError;
use crate::matrix::view_projection;
use crate::overlay::{
    ATLAS_HEIGHT, ATLAS_WIDTH, MAX_VERTICES, VERTEX_FLOATS, font_atlas, overlay_vertices,
};
use crate::renderer::{Capture, Target, View};
use crate::visible::VisibleSet;
use crate::world::{BatchKind, Vertex, WorldMesh};

/// The colour format of every target. UNORM keeps the original's sums.
const COLOR_FORMAT: Format = Format::B8G8R8A8_UNORM;

/// The depth format.
const DEPTH_FORMAT: Format = Format::D32_SFLOAT;

/// The colour behind everything, seen with no map loaded.
const CLEAR_COLOR: [f32; 4] = [0.02, 0.02, 0.03, 1.0];

/// The `VK_INDEX_TYPE_UINT32` value.
const INDEX_TYPE_U32: u32 = 1;

/// Bytes in the frame uniforms: a matrix, two vectors and 64 styles.
const FRAME_SIZE: usize = 64 + 16 + 16 + 256;

/// Texture frames per second, from `R_TextureAnimation`.
const ANIMATION_RATE: f32 = 5.0;

/// The most windows' worth of swapchain images expected.
const MIN_SWAPCHAIN_IMAGES: u32 = 2;

/// The world shader.
const WORLD_SHADER: &str = include_str!("shaders/world.wgsl");

/// The overlay shader.
const OVERLAY_SHADER: &str = include_str!("shaders/overlay.wgsl");

/// What [`Backend`] needs to draw a loaded map.
pub(crate) struct Scene<'a> {
    /// The map.
    pub(crate) map: &'a Map,
    /// Its mesh.
    pub(crate) mesh: &'a WorldMesh,
    /// What's visible.
    pub(crate) visible: &'a VisibleSet,
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
    /// Kept alive for the sets.
    _textures: Vec<GpuImage>,
    /// Kept alive for the sets.
    _pool: DescriptorPool,
    /// The lightmap set.
    frame_set: DescriptorSet,
    /// Kept alive for the set.
    _atlas: GpuImage,
}

/// The text over the picture.
struct OverlayPass {
    /// The pipeline.
    pipeline: GraphicsPipeline,
    /// Its layout.
    layout: PipelineLayout,
    /// The font's set.
    set: DescriptorSet,
    /// Kept alive for the set.
    _pool: DescriptorPool,
    /// Kept alive for the set.
    _set_layout: DescriptorSetLayout,
    /// Kept alive for the set.
    _sampler: Sampler,
    /// Kept alive for the set.
    _font: GpuImage,
    /// The vertices, rewritten only when the text or size changes.
    vertices: GpuBuffer,
    /// How many vertices to draw.
    count: u32,
    /// What the vertices were built for.
    built_for: Option<(String, (u32, u32))>,
}

/// The four pipelines.
struct Pipelines {
    /// Lit surfaces.
    solid: GraphicsPipeline,
    /// Lit surfaces with holes.
    cutout: GraphicsPipeline,
    /// Liquids.
    liquid: GraphicsPipeline,
    /// Sky.
    sky: GraphicsPipeline,
}

impl Pipelines {
    /// The pipeline for `kind`.
    fn get(&self, kind: BatchKind) -> &GraphicsPipeline {
        match kind {
            BatchKind::Solid => &self.solid,
            BatchKind::Cutout => &self.cutout,
            BatchKind::Liquid => &self.liquid,
            BatchKind::Sky => &self.sky,
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
    /// The text to show.
    overlay_text: String,
    /// What draws it, set up just after the rest.
    overlay: Option<OverlayPass>,
    /// The frame uniforms.
    uniforms: GpuBuffer,
    /// The frame set layout.
    frame_layout: DescriptorSetLayout,
    /// The texture set layout.
    texture_layout: DescriptorSetLayout,
    /// The set used with no map.
    pipeline_layout: PipelineLayout,
    /// The pipelines.
    pipelines: Pipelines,
    /// The pass ending ready to present.
    present_pass: RenderPass,
    /// The pass ending ready to read back.
    capture_pass: RenderPass,
    /// The texture sampler.
    texture_sampler: Sampler,
    /// The lightmap sampler.
    light_sampler: Sampler,
    /// The buffer being recorded or in flight.
    commands: Option<CommandBuffer>,
    /// Commands for a frame.
    command_pool: CommandPool,
    /// Signals when an image is ready to draw to.
    available: Semaphore,
    /// Signals when the last frame finished.
    in_flight: Option<Fence>,
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
    Err(RenderError("no Vulkan device can draw here".to_owned()))
}

/// A render pass with one colour and one depth attachment.
fn make_pass(device: &Device, final_layout: ImageLayout) -> Result<RenderPass, RenderError> {
    Ok(RenderPass::new(
        device,
        RenderPassCreateInfo {
            color_attachments: &[AttachmentDescription {
                format: COLOR_FORMAT,
                load_op: AttachmentLoadOp::CLEAR,
                store_op: AttachmentStoreOp::STORE,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout,
            }],
            depth_attachment: Some(AttachmentDescription {
                format: DEPTH_FORMAT,
                load_op: AttachmentLoadOp::CLEAR,
                store_op: AttachmentStoreOp::DONT_CARE,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            }),
        },
    )?)
}

/// A buffer the CPU can write.
fn host_buffer(
    device: &Device,
    physical: &PhysicalDevice,
    size: usize,
    usage: BufferUsage,
) -> Result<GpuBuffer, RenderError> {
    let (buffer, memory) = Buffer::new_bound(
        device,
        physical,
        BufferCreateInfo {
            size: size.max(4) as u64,
            usage,
        },
        MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
    )?;
    Ok(GpuBuffer { buffer, memory })
}

/// An image in device memory.
fn device_image(
    device: &Device,
    physical: &PhysicalDevice,
    image: Image,
    depth: bool,
) -> Result<GpuImage, RenderError> {
    let requirements = image.memory_requirements();
    let memory_type = physical
        .find_memory_type(
            requirements.memory_type_bits,
            MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .ok_or_else(|| RenderError("no memory for an image".to_owned()))?;
    let memory = DeviceMemory::allocate(device, requirements.size, memory_type)?;
    image.bind_memory(&memory, 0)?;
    let view = if depth {
        ImageView::new_2d_depth(&image)?
    } else {
        ImageView::new_2d_color(&image)?
    };
    Ok(GpuImage {
        view,
        image,
        _memory: memory,
    })
}

/// A depth image of `size`.
fn depth_image(
    device: &Device,
    physical: &PhysicalDevice,
    size: (u32, u32),
) -> Result<GpuImage, RenderError> {
    let image = Image::new_2d(
        device,
        Image2dCreateInfo {
            format: DEPTH_FORMAT,
            width: size.0,
            height: size.1,
            usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
        },
    )?;
    device_image(device, physical, image, true)
}

/// Writes `view`'s uniforms into `out`.
fn frame_bytes(view: &View, aspect: f32) -> Vec<u8> {
    let matrix = view_projection(view.origin, view.yaw, view.pitch, view.fov, aspect);
    let eye = [view.origin.x, view.origin.y, view.origin.z, 1.0];
    let params = [view.time, 0.0, 0.0, 0.0];
    let mut out = Vec::with_capacity(FRAME_SIZE);
    for value in matrix.iter().chain(&eye).chain(&params).chain(&view.styles) {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
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
        })?;
        let surface = window
            .map(|window| make_surface(&instance, window).map(Arc::new))
            .transpose()?;
        let (physical, queue_family) = pick_device(&instance, surface.as_deref())?;
        let device_name = physical.properties().device_name().to_owned();
        let device_extensions = DeviceExtensions::new().khr_swapchain();
        let device = physical.create_device(DeviceCreateInfo {
            queue_create_infos: &[QueueCreateInfo::single(queue_family)],
            enabled_extensions: surface.as_ref().map(|_| &device_extensions),
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
            ],
        )?;
        let texture_layout = DescriptorSetLayout::new(
            &device,
            &[
                image_binding(0, DescriptorType::SAMPLED_IMAGE),
                image_binding(1, DescriptorType::SAMPLER),
            ],
        )?;
        let pipeline_layout = PipelineLayout::new(&device, &[&frame_layout, &texture_layout])?;
        let pipelines = Self::make_pipelines(&device, &pipeline_layout, &present_pass)?;
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
            3.0,
        )?;
        let light_sampler = Sampler::new(&device, SamplerCreateInfo::default())?;
        let uniforms = host_buffer(&device, &physical, FRAME_SIZE, BufferUsage::UNIFORM_BUFFER)?;
        let command_pool = CommandPool::new(&device, queue_family)?;
        let available = Semaphore::binary(&device)?;
        let mut backend = Self {
            surface,
            window: None,
            wanted_size: size,
            resized: false,
            map: None,
            overlay_text: String::new(),
            overlay: None,
            uniforms,
            frame_layout,
            texture_layout,
            pipeline_layout,
            pipelines,
            present_pass,
            capture_pass,
            texture_sampler,
            light_sampler,
            command_pool,
            commands: None,
            available,
            in_flight: None,
            present_mode,
            free_present_mode: present_mode,
            queue,
            queue_family,
            device_name,
            physical,
            device,
            _instance: instance,
        };
        backend.overlay = Some(backend.make_overlay()?);
        backend.rebuild_window()?;
        Ok(backend)
    }

    /// Sets up the overlay's font and pipeline.
    fn make_overlay(&self) -> Result<OverlayPass, RenderError> {
        let font = font_atlas();
        let image = self.upload_image(
            &[(ATLAS_WIDTH, ATLAS_HEIGHT, font.as_slice())],
            Format::R8G8B8A8_UNORM,
        )?;
        let sampler = Sampler::new(
            &self.device,
            SamplerCreateInfo {
                mag_filter: SamplerFilter::NEAREST,
                min_filter: SamplerFilter::NEAREST,
                ..SamplerCreateInfo::default()
            },
        )?;
        let binding = |binding, descriptor_type| DescriptorSetLayoutBinding {
            binding,
            descriptor_type,
            descriptor_count: 1,
            stage_flags: ShaderStageFlags::FRAGMENT,
        };
        let set_layout = DescriptorSetLayout::new(
            &self.device,
            &[
                binding(0, DescriptorType::SAMPLED_IMAGE),
                binding(1, DescriptorType::SAMPLER),
            ],
        )?;
        let pool = DescriptorPool::new(
            &self.device,
            1,
            &[
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: 1,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLER,
                    descriptor_count: 1,
                },
            ],
        )?;
        let set = pool.allocate(&set_layout)?;
        set.write_sampled_image(0, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        set.write_sampler(1, &sampler);
        let layout = PipelineLayout::new(&self.device, &[&set_layout])?;
        let spirv = vulkane::safe::naga::compile_wgsl(OVERLAY_SHADER)
            .map_err(|error| RenderError(error.to_string()))?;
        let shader = ShaderModule::from_spirv(&self.device, &spirv)?;
        let stride = u32::try_from(VERTEX_FLOATS * 4).unwrap_or(0);
        let pipeline = GraphicsPipelineBuilder::new(&layout, &self.present_pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_main")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_main")
            .vertex_input(
                &[VertexInputBinding {
                    binding: 0,
                    stride,
                    input_rate: InputRate::VERTEX,
                }],
                &[
                    VertexInputAttribute {
                        location: 0,
                        binding: 0,
                        format: Format::R32G32_SFLOAT,
                        offset: 0,
                    },
                    VertexInputAttribute {
                        location: 1,
                        binding: 0,
                        format: Format::R32G32_SFLOAT,
                        offset: 8,
                    },
                ],
            )
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(false, false)
            .alpha_blending(true)
            .build(&self.device)?;
        let vertices = host_buffer(
            &self.device,
            &self.physical,
            MAX_VERTICES * VERTEX_FLOATS * 4,
            BufferUsage::VERTEX_BUFFER,
        )?;
        Ok(OverlayPass {
            pipeline,
            layout,
            set,
            _pool: pool,
            _set_layout: set_layout,
            _sampler: sampler,
            _font: image,
            vertices,
            count: 0,
            built_for: None,
        })
    }

    /// Sets the overlay's text.
    pub(crate) fn set_overlay_text(&mut self, text: &str) {
        text.clone_into(&mut self.overlay_text);
    }

    /// Rebuilds the overlay's vertices if its text or size changed.
    fn upload_overlay(&mut self, size: (u32, u32)) -> Result<(), RenderError> {
        let Some(overlay) = &mut self.overlay else {
            return Ok(());
        };
        let current = overlay
            .built_for
            .as_ref()
            .is_some_and(|(text, built)| *text == self.overlay_text && *built == size);
        if current {
            return Ok(());
        }
        let floats = overlay_vertices(&self.overlay_text, size);
        let mut mapped = overlay.vertices.memory.map()?;
        let (target, _remainder) = mapped.as_slice_mut().as_chunks_mut::<4>();
        for (value, chunk) in floats.iter().zip(target) {
            *chunk = value.to_le_bytes();
        }
        drop(mapped);
        overlay.count = u32::try_from(floats.len() / VERTEX_FLOATS).unwrap_or(0);
        overlay.built_for = Some((self.overlay_text.clone(), size));
        Ok(())
    }

    /// Records the overlay's one draw.
    fn record_overlay(
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        overlay: Option<&OverlayPass>,
        size: (u32, u32),
    ) {
        let Some(overlay) = overlay.filter(|overlay| overlay.count > 0) else {
            return;
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        rec.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32);
        rec.set_scissor(0, 0, size.0, size.1);
        rec.bind_graphics_pipeline(&overlay.pipeline);
        rec.bind_graphics_descriptor_sets(&overlay.layout, 0, &[&overlay.set]);
        rec.bind_vertex_buffers(0, &[(&overlay.vertices.buffer, 0)]);
        rec.draw(overlay.count, 1, 0, 0);
    }

    /// The four pipelines, which differ in their fragment stage.
    fn make_pipelines(
        device: &Device,
        layout: &PipelineLayout,
        pass: &RenderPass,
    ) -> Result<Pipelines, RenderError> {
        let spirv = vulkane::safe::naga::compile_wgsl(WORLD_SHADER)
            .map_err(|error| RenderError(error.to_string()))?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        let bindings = [VertexInputBinding {
            binding: 0,
            stride: u32::try_from(Vertex::SIZE).unwrap_or(0),
            input_rate: InputRate::VERTEX,
        }];
        let attribute = |location, format, offset| VertexInputAttribute {
            location,
            binding: 0,
            format,
            offset,
        };
        let attributes = [
            attribute(0, Format::R32G32B32_SFLOAT, 0),
            attribute(1, Format::R32G32_SFLOAT, 12),
            attribute(2, Format::R32G32_SFLOAT, 20),
            attribute(3, Format::R32_SFLOAT, 28),
            attribute(4, Format::R8G8B8A8_UINT, 32),
        ];
        let pipeline = |entry| {
            GraphicsPipelineBuilder::new(layout, pass)
                .stage(GraphicsShaderStage::Vertex, &shader, "vs_main")
                .stage(GraphicsShaderStage::Fragment, &shader, entry)
                .vertex_input(&bindings, &attributes)
                .dynamic_viewport_scissor()
                .cull_mode(CullMode::NONE)
                .depth_test(true, true)
                .build(device)
        };
        Ok(Pipelines {
            solid: pipeline("fs_solid")?,
            cutout: pipeline("fs_cutout")?,
            liquid: pipeline("fs_liquid")?,
            sky: pipeline("fs_sky")?,
        })
    }

    /// The GPU's name.
    pub(crate) fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Waits for the frame in flight.
    fn wait_for_frame(&mut self) -> Result<(), RenderError> {
        if let Some(fence) = self.in_flight.take() {
            fence.wait(u64::MAX)?;
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

    /// Notes the window's new size.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.wanted_size = (width, height);
        self.resized = true;
    }

    /// Uploads `map`.
    pub(crate) fn load_map(&mut self, map: &Map, mesh: &WorldMesh) -> Result<(), RenderError> {
        self.wait_for_frame()?;
        self.device.wait_idle()?;
        self.map = None;
        let mut vertex_bytes = Vec::with_capacity(mesh.vertices.len() * Vertex::SIZE);
        for vertex in &mesh.vertices {
            vertex.write(&mut vertex_bytes);
        }
        let vertices = self.device_buffer(&vertex_bytes, BufferUsage::VERTEX_BUFFER)?;
        let index_count: usize = mesh.face_indices.iter().map(Vec::len).sum();
        let indices = host_buffer(
            &self.device,
            &self.physical,
            index_count * 4,
            BufferUsage::INDEX_BUFFER,
        )?;

        let sets = u32::try_from(map.textures.len() + 1).unwrap_or(u32::MAX);
        let pool = DescriptorPool::new(
            &self.device,
            sets,
            &[
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::UNIFORM_BUFFER,
                    descriptor_count: 1,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: sets,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLER,
                    descriptor_count: sets,
                },
            ],
        )?;
        let atlas = self.upload_image(
            &[(mesh.atlas_size, mesh.atlas_size, mesh.atlas.as_slice())],
            Format::R8G8B8A8_UNORM,
        )?;
        let frame_set = pool.allocate(&self.frame_layout)?;
        frame_set.write_buffer(
            0,
            DescriptorType::UNIFORM_BUFFER,
            &self.uniforms.buffer,
            0,
            FRAME_SIZE as u64,
        );
        frame_set.write_sampled_image(1, &atlas.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        frame_set.write_sampler(2, &self.light_sampler);

        let mut textures = Vec::with_capacity(map.textures.len());
        let mut texture_sets = Vec::with_capacity(map.textures.len());
        for texture in &map.textures {
            if texture.kind == TextureKind::Hidden {
                texture_sets.push(None);
                continue;
            }
            let levels: Vec<(u32, u32, &[u8])> = texture
                .mips
                .iter()
                .map(|mip| (mip.width, mip.height, mip.rgba.as_slice()))
                .collect();
            let image = self.upload_image(&levels, Format::R8G8B8A8_UNORM)?;
            let set = pool.allocate(&self.texture_layout)?;
            set.write_sampled_image(0, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            set.write_sampler(1, &self.texture_sampler);
            texture_sets.push(Some(set));
            textures.push(image);
        }
        self.map = Some(GpuMap {
            vertices,
            indices,
            texture_sets,
            _textures: textures,
            _pool: pool,
            frame_set,
            _atlas: atlas,
        });
        Ok(())
    }

    /// A device-local buffer holding `bytes`.
    fn device_buffer(&self, bytes: &[u8], usage: BufferUsage) -> Result<GpuBuffer, RenderError> {
        let mut staging = host_buffer(
            &self.device,
            &self.physical,
            bytes.len(),
            BufferUsage::TRANSFER_SRC,
        )?;
        staging.memory.map()?.as_slice_mut()[..bytes.len()].copy_from_slice(bytes);
        let (buffer, memory) = Buffer::new_bound(
            &self.device,
            &self.physical,
            BufferCreateInfo {
                size: bytes.len().max(4) as u64,
                usage: usage | BufferUsage::TRANSFER_DST,
            },
            MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
                rec.copy_buffer(
                    &staging.buffer,
                    &buffer,
                    &[BufferCopy {
                        src_offset: 0,
                        dst_offset: 0,
                        size: bytes.len().max(4) as u64,
                    }],
                );
                Ok(())
            })?;
        Ok(GpuBuffer { buffer, memory })
    }

    /// An image holding `levels`, largest first, ready to sample.
    fn upload_image(
        &self,
        levels: &[(u32, u32, &[u8])],
        format: Format,
    ) -> Result<GpuImage, RenderError> {
        let &(width, height, _) = levels
            .first()
            .ok_or_else(|| RenderError("an image with no pixels".to_owned()))?;
        let image = Image::new_2d_array(
            &self.device,
            Image2dCreateInfo {
                format,
                width,
                height,
                usage: ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
            },
            1,
            u32::try_from(levels.len()).unwrap_or(1),
        )?;
        let image = device_image(&self.device, &self.physical, image, false)?;
        let mut pixels = Vec::new();
        let mut regions = Vec::with_capacity(levels.len());
        for (level, &(width, height, bytes)) in levels.iter().enumerate() {
            let mut copy = BufferImageCopy::full_2d(width, height);
            copy.buffer_offset = pixels.len() as u64;
            copy.mip_level = u32::try_from(level).unwrap_or(0);
            regions.push(copy);
            pixels.extend_from_slice(bytes);
        }
        let mut staging = host_buffer(
            &self.device,
            &self.physical,
            pixels.len(),
            BufferUsage::TRANSFER_SRC,
        )?;
        staging.memory.map()?.as_slice_mut()[..pixels.len()].copy_from_slice(&pixels);
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
                rec.image_barrier(
                    PipelineStage::TOP_OF_PIPE,
                    PipelineStage::TRANSFER,
                    ImageBarrier::color(
                        &image.image,
                        ImageLayout::UNDEFINED,
                        ImageLayout::TRANSFER_DST_OPTIMAL,
                        AccessFlags::NONE,
                        AccessFlags::TRANSFER_WRITE,
                    ),
                );
                rec.copy_buffer_to_image(
                    &staging.buffer,
                    &image.image,
                    ImageLayout::TRANSFER_DST_OPTIMAL,
                    &regions,
                );
                rec.image_barrier(
                    PipelineStage::TRANSFER,
                    PipelineStage::FRAGMENT_SHADER,
                    ImageBarrier::color(
                        &image.image,
                        ImageLayout::TRANSFER_DST_OPTIMAL,
                        ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        AccessFlags::TRANSFER_WRITE,
                        AccessFlags::SHADER_READ,
                    ),
                );
                Ok(())
            })?;
        Ok(image)
    }

    /// Writes the uniforms, and the indices if they changed.
    fn upload_frame(
        &mut self,
        view: &View,
        scene: Option<&Scene<'_>>,
        aspect: f32,
    ) -> Result<(), RenderError> {
        let bytes = frame_bytes(view, aspect);
        self.uniforms.memory.map()?.as_slice_mut()[..bytes.len()].copy_from_slice(&bytes);
        if let (Some(scene), Some(map)) = (scene, &mut self.map)
            && scene.changed
        {
            let mut mapped = map.indices.memory.map()?;
            let (target, _remainder) = mapped.as_slice_mut().as_chunks_mut::<4>();
            for (index, chunk) in scene.visible.indices.iter().zip(target) {
                *chunk = index.to_le_bytes();
            }
        }
        Ok(())
    }

    /// Records the draw calls of `scene`.
    fn record_world(
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        layout: &PipelineLayout,
        pipelines: &Pipelines,
        map: &GpuMap,
        scene: &Scene<'_>,
        time: f32,
        size: (u32, u32),
    ) {
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        rec.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32);
        rec.set_scissor(0, 0, size.0, size.1);
        rec.bind_vertex_buffers(0, &[(&map.vertices.buffer, 0)]);
        rec.bind_index_buffer(&map.indices.buffer, 0, INDEX_TYPE_U32);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "time is never negative, and the frame count wraps by design"
        )]
        let step = (time * ANIMATION_RATE) as u32;
        let mut bound = None;
        for (batch, &(first, count)) in scene.mesh.batches.iter().zip(&scene.visible.ranges) {
            if count == 0 {
                continue;
            }
            let texture = animated_texture(scene.map, batch.texture, step);
            let Some(Some(set)) = map.texture_sets.get(texture as usize) else {
                continue;
            };
            if bound != Some(batch.kind) {
                rec.bind_graphics_pipeline(pipelines.get(batch.kind));
                bound = Some(batch.kind);
            }
            rec.bind_graphics_descriptor_sets(layout, 0, &[&map.frame_set, set]);
            rec.draw_indexed(count, 1, first, 0, 0);
        }
    }

    /// Draws to the window.
    pub(crate) fn draw(
        &mut self,
        view: &View,
        scene: Option<&Scene<'_>>,
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
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        let aspect = extent.0 as f32 / extent.1 as f32;
        self.upload_frame(view, scene, aspect)?;
        self.upload_overlay(extent)?;
        let Some(window) = &self.window else {
            return Ok(());
        };
        let at = image as usize;
        let mut commands = self.command_pool.allocate_primary()?;
        {
            let mut rec = commands.begin()?;
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
            if let (Some(scene), Some(map)) = (scene, &self.map) {
                Self::record_world(
                    &mut rec,
                    &self.pipeline_layout,
                    &self.pipelines,
                    map,
                    scene,
                    view.time,
                    extent,
                );
            }
            Self::record_overlay(&mut rec, self.overlay.as_ref(), extent);
            rec.end_render_pass();
            rec.end()?;
        }
        let fence = Fence::new(&self.device)?;
        self.queue.submit_with_sync(
            &[&commands],
            &[WaitSemaphore::binary(
                &self.available,
                PipelineStage::COLOR_ATTACHMENT_OUTPUT,
            )],
            &[SignalSemaphore::binary(&window.finished[at])],
            Some(&fence),
        )?;
        let presented = window
            .swapchain
            .present(&self.queue, image, &[&window.finished[at]]);
        self.commands = Some(commands);
        self.in_flight = Some(fence);
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
        self.upload_overlay((width, height))?;
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
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
                if let (Some(scene), Some(map)) = (scene, &self.map) {
                    Self::record_world(
                        rec,
                        &self.pipeline_layout,
                        &self.pipelines,
                        map,
                        scene,
                        view.time,
                        (width, height),
                    );
                }
                Self::record_overlay(rec, self.overlay.as_ref(), (width, height));
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

/// The texture to show for `texture` at animation `step`.
fn animated_texture(map: &Map, texture: u32, step: u32) -> u32 {
    map.textures
        .get(texture as usize)
        .and_then(|texture| texture.animation.as_ref())
        .and_then(|animation| {
            let count = u32::try_from(animation.frames.len()).ok()?;
            let at = (animation.position + step) % count;
            animation.frames.get(at as usize).copied()
        })
        .unwrap_or(texture)
}

impl Drop for Backend {
    fn drop(&mut self) {
        let _ignored = self.device.wait_idle();
    }
}
