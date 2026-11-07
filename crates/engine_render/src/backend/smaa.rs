//! SMAA 1x's first two passes: edges, then how much each side of one takes from
//! the other. The final pass blends by what they leave (proposal 0093).

use std::cell::Cell;

use engine_shaders::{Module, spirv};
use vulkane::safe::{
    AccessFlags, AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, ClearValue, CullMode,
    DescriptorPool, DescriptorPoolSize, DescriptorSet, DescriptorSetLayout,
    DescriptorSetLayoutBinding, DescriptorType, Device, Format, Framebuffer, GraphicsPipeline,
    GraphicsPipelineBuilder, GraphicsShaderStage, Image, Image2dCreateInfo, ImageLayout,
    ImageUsage, PhysicalDevice, PipelineLayout, PipelineStage, RenderPass, RenderPassCreateInfo,
    ShaderModule, ShaderStageFlags,
};

use super::GpuImage;
use super::upload::device_image;
use crate::error::RenderError;

/// The format of both images.
const FORMAT: Format = Format::R8G8B8A8_UNORM;

/// One image a pass draws, and what it reads.
struct Stage {
    /// What it draws to.
    framebuffer: Framebuffer,
    /// What it reads.
    set: DescriptorSet,
    /// What it draws.
    image: GpuImage,
}

/// SMAA's images for a scene of one size.
pub(super) struct SmaaTarget {
    /// Finds edges.
    edges: Stage,
    /// Works out weights.
    weights: Stage,
    /// Kept alive for the sets.
    _pool: DescriptorPool,
    /// Whether the images have been started off.
    ready: Cell<bool>,
}

/// What SMAA's passes need, whatever the size.
pub(super) struct Smaa {
    /// Draws an image, leaving it ready to read.
    pass: RenderPass,
    /// The layout of what a pass reads.
    set_layout: DescriptorSetLayout,
    /// The layout of its draws.
    layout: PipelineLayout,
    /// Finds edges.
    edges: GraphicsPipeline,
    /// Works out weights.
    weights: GraphicsPipeline,
}

impl Smaa {
    /// Makes the passes.
    pub(super) fn new(device: &Device) -> Result<Self, RenderError> {
        let pass = RenderPass::new(
            device,
            RenderPassCreateInfo {
                color_attachments: &[AttachmentDescription {
                    format: FORMAT,
                    load_op: AttachmentLoadOp::CLEAR,
                    store_op: AttachmentStoreOp::STORE,
                    initial_layout: ImageLayout::UNDEFINED,
                    final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                }],
                depth_attachment: None,
            },
        )?;
        let set_layout = DescriptorSetLayout::new(
            device,
            &[DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: DescriptorType::SAMPLED_IMAGE,
                descriptor_count: 1,
                stage_flags: ShaderStageFlags::FRAGMENT,
            }],
        )?;
        let layout = PipelineLayout::new(device, &[&set_layout])?;
        let spirv = spirv(&Module::Smaa).map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        let pipeline = |entry| {
            GraphicsPipelineBuilder::new(&layout, &pass)
                .stage(GraphicsShaderStage::Vertex, &shader, "vs_full")
                .stage(GraphicsShaderStage::Fragment, &shader, entry)
                .dynamic_viewport_scissor()
                .cull_mode(CullMode::NONE)
                .depth_test(false, false)
                .build(device)
        };
        let edges = pipeline("fs_edges")?;
        let weights = pipeline("fs_weights")?;
        Ok(Self {
            pass,
            set_layout,
            layout,
            edges,
            weights,
        })
    }

    /// Makes one stage's image, reading `source`.
    fn stage(
        &self,
        device: &Device,
        physical: &PhysicalDevice,
        pool: &DescriptorPool,
        size: (u32, u32),
        source: &GpuImage,
    ) -> Result<Stage, RenderError> {
        let image = Image::new_2d(
            device,
            Image2dCreateInfo {
                format: FORMAT,
                width: size.0,
                height: size.1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
            },
        )?;
        let image = device_image(device, physical, image, false)?;
        let framebuffer = Framebuffer::new(device, &self.pass, &[&image.view], size.0, size.1)?;
        let set = pool.allocate(&self.set_layout)?;
        set.write_sampled_image(0, &source.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        Ok(Stage {
            framebuffer,
            set,
            image,
        })
    }

    /// Makes the images for a scene of `size` whose colour is `scene`.
    pub(super) fn target(
        &self,
        device: &Device,
        physical: &PhysicalDevice,
        size: (u32, u32),
        scene: &GpuImage,
    ) -> Result<SmaaTarget, RenderError> {
        let pool = DescriptorPool::new(
            device,
            2,
            &[DescriptorPoolSize {
                descriptor_type: DescriptorType::SAMPLED_IMAGE,
                descriptor_count: 2,
            }],
        )?;
        let edges = self.stage(device, physical, &pool, size, scene)?;
        let weights = self.stage(device, physical, &pool, size, &edges.image)?;
        Ok(SmaaTarget {
            edges,
            weights,
            _pool: pool,
            ready: Cell::new(false),
        })
    }

    /// Runs one stage. With `pipeline` as `None` it only empties its image.
    fn record_stage(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        stage: &Stage,
        pipeline: Option<&GraphicsPipeline>,
        size: (u32, u32),
    ) {
        let empty = [ClearValue::Color([0.0; 4])];
        rec.begin_render_pass_ext(&self.pass, &stage.framebuffer, &empty);
        if let Some(pipeline) = pipeline {
            #[expect(
                clippy::cast_precision_loss,
                reason = "window sizes are far below 2^24"
            )]
            rec.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32);
            rec.set_scissor(0, 0, size.0, size.1);
            rec.bind_graphics_pipeline(pipeline);
            rec.bind_graphics_descriptor_sets(&self.layout, 0, &[&stage.set]);
            rec.draw(3, 1, 0, 0);
        }
        rec.end_render_pass();
        rec.memory_barrier(
            PipelineStage::ALL_GRAPHICS,
            PipelineStage::FRAGMENT_SHADER,
            AccessFlags::COLOR_ATTACHMENT_WRITE,
            AccessFlags::SHADER_READ,
        );
    }

    /// Records the passes if `wanted`. Otherwise it only makes sure the final
    /// pass has something it can read.
    pub(super) fn record(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        target: &SmaaTarget,
        size: (u32, u32),
        wanted: bool,
    ) {
        let started = target.ready.replace(true);
        if wanted {
            self.record_stage(rec, &target.edges, Some(&self.edges), size);
            self.record_stage(rec, &target.weights, Some(&self.weights), size);
        } else if !started {
            self.record_stage(rec, &target.edges, None, size);
            self.record_stage(rec, &target.weights, None, size);
        }
    }
}

impl SmaaTarget {
    /// The weights, for the final pass to read.
    pub(super) const fn weights(&self) -> &GpuImage {
        &self.weights.image
    }
}
