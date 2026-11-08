//! Bringing the scene's samples to one a pixel (proposal 0151). Each is tamed, as the screen
//! will show it, before they're averaged, so that one brighter than the screen shows doesn't
//! outweigh the rest once it's brought down to fit, and something thin and dark before it stays.

use engine_shaders::{Module, spirv};
use vulkane::safe::{
    AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, Buffer, CullMode, DescriptorPool,
    DescriptorPoolSize, DescriptorSet, DescriptorSetLayout, DescriptorSetLayoutBinding,
    DescriptorType, Device, Framebuffer, GraphicsPipeline, GraphicsPipelineBuilder,
    GraphicsShaderStage, ImageLayout, ImageView, PipelineLayout, RenderPass, RenderPassCreateInfo,
    ShaderModule, ShaderStageFlags,
};

use super::{FRAME_SIZE, SCENE_FORMAT};
use crate::error::RenderError;

/// What brings samples to one: its pass, which draws every pixel, and its pipeline.
pub(super) struct Resolver {
    /// Its pass.
    pass: RenderPass,
    /// Its set's layout: the frame, and the scene's samples.
    set_layout: DescriptorSetLayout,
    /// Its pipeline's.
    layout: PipelineLayout,
    /// Its pipeline.
    pipeline: GraphicsPipeline,
}

/// What a scene with samples is brought to one with.
pub(super) struct ResolveTarget {
    /// The image a pixel each, which the final pass reads.
    framebuffer: Framebuffer,
    /// The frame and the samples.
    set: DescriptorSet,
    /// Kept alive for the set.
    _pool: DescriptorPool,
    /// Its size.
    size: (u32, u32),
}

impl Resolver {
    /// Makes it.
    pub(super) fn new(device: &Device) -> Result<Self, RenderError> {
        let pass = RenderPass::new(
            device,
            RenderPassCreateInfo {
                color_attachments: &[AttachmentDescription {
                    format: SCENE_FORMAT,
                    load_op: AttachmentLoadOp::DONT_CARE,
                    store_op: AttachmentStoreOp::STORE,
                    initial_layout: ImageLayout::UNDEFINED,
                    final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                }],
                depth_attachment: None,
            },
        )?;
        let binding = |binding, descriptor_type| DescriptorSetLayoutBinding {
            binding,
            descriptor_type,
            descriptor_count: 1,
            stage_flags: ShaderStageFlags::FRAGMENT,
        };
        let set_layout = DescriptorSetLayout::new(
            device,
            &[
                binding(0, DescriptorType::UNIFORM_BUFFER),
                binding(1, DescriptorType::SAMPLED_IMAGE),
            ],
        )?;
        let layout = PipelineLayout::new(device, &[&set_layout])?;
        let spirv = spirv(&Module::Resolve).map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        let pipeline = GraphicsPipelineBuilder::new(&layout, &pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_resolve")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_resolve")
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(false, false)
            .build(device)?;
        Ok(Self {
            pass,
            set_layout,
            layout,
            pipeline,
        })
    }

    /// What brings `samples`, of `size`, to one a pixel in `into`, with the frame in `frame`.
    pub(super) fn target(
        &self,
        device: &Device,
        (samples, into): (&ImageView, &ImageView),
        frame: &Buffer,
        size: (u32, u32),
    ) -> Result<ResolveTarget, RenderError> {
        let framebuffer = Framebuffer::new(device, &self.pass, &[into], size.0, size.1)?;
        let pool = DescriptorPool::new(
            device,
            1,
            &[
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::UNIFORM_BUFFER,
                    descriptor_count: 1,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: 1,
                },
            ],
        )?;
        let set = pool.allocate(&self.set_layout)?;
        set.write_buffer(
            0,
            DescriptorType::UNIFORM_BUFFER,
            frame,
            0,
            FRAME_SIZE as u64,
        );
        set.write_sampled_image(1, samples, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        Ok(ResolveTarget {
            framebuffer,
            set,
            _pool: pool,
            size,
        })
    }

    /// Records bringing the scene's samples to one, after everything's drawn into them.
    pub(super) fn record(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        target: &ResolveTarget,
    ) {
        rec.begin_render_pass_ext(&self.pass, &target.framebuffer, &[]);
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        rec.set_viewport(0.0, 0.0, target.size.0 as f32, target.size.1 as f32);
        rec.set_scissor(0, 0, target.size.0, target.size.1);
        rec.bind_graphics_pipeline(&self.pipeline);
        rec.bind_graphics_descriptor_sets(&self.layout, 0, &[&target.set]);
        rec.draw(3, 1, 0, 0);
        rec.end_render_pass();
    }
}
