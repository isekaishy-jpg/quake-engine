//! Drawing the scene off-screen, so a final pass can read its colour and depth
//! (proposal 0075).

use vulkane::safe::{
    AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, CullMode, DescriptorPool,
    DescriptorPoolSize, DescriptorSet, DescriptorType, Device, Framebuffer, GraphicsPipeline,
    GraphicsPipelineBuilder, GraphicsShaderStage, Image, Image2dCreateInfo, ImageLayout,
    ImageUsage, PipelineLayout, RenderPass, RenderPassCreateInfo, ShaderModule,
};

use super::upload::device_image;
use super::{Backend, COLOR_FORMAT, DEPTH_FORMAT, FRAME_SIZE, GpuImage, POST_SHADER};
use crate::error::RenderError;

/// Where a scene is drawn before the final pass.
pub(super) struct SceneTarget {
    /// The final pass's set.
    pub(super) set: DescriptorSet,
    /// Kept alive for the set.
    _pool: DescriptorPool,
    /// Where the scene is drawn.
    pub(super) framebuffer: Framebuffer,
    /// Kept alive for the framebuffer.
    _color: GpuImage,
    /// Kept alive for the framebuffer.
    _depth: GpuImage,
    /// Its size.
    pub(super) size: (u32, u32),
}

/// The render pass of a scene, leaving both attachments ready to read.
pub(super) fn make_scene_pass(device: &Device) -> Result<RenderPass, RenderError> {
    Ok(RenderPass::new(
        device,
        RenderPassCreateInfo {
            color_attachments: &[AttachmentDescription {
                format: COLOR_FORMAT,
                load_op: AttachmentLoadOp::CLEAR,
                store_op: AttachmentStoreOp::STORE,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }],
            depth_attachment: Some(AttachmentDescription {
                format: DEPTH_FORMAT,
                load_op: AttachmentLoadOp::CLEAR,
                store_op: AttachmentStoreOp::STORE,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }),
        },
    )?)
}

impl Backend {
    /// The final pass's pipeline.
    pub(super) fn make_post_pipeline(
        device: &Device,
        layout: &PipelineLayout,
        pass: &RenderPass,
    ) -> Result<GraphicsPipeline, RenderError> {
        let spirv = vulkane::safe::naga::compile_wgsl(POST_SHADER)
            .map_err(|error| RenderError(error.to_string()))?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        Ok(GraphicsPipelineBuilder::new(layout, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_post")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_post")
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(false, false)
            .build(device)?)
    }

    /// Makes sure the scene target is `size`.
    pub(super) fn ensure_scene_target(&mut self, size: (u32, u32)) -> Result<(), RenderError> {
        if self
            .scene_target
            .as_ref()
            .is_some_and(|target| target.size == size)
        {
            return Ok(());
        }
        self.device.wait_idle()?;
        self.scene_target = None;
        let color = Image::new_2d(
            &self.device,
            Image2dCreateInfo {
                format: COLOR_FORMAT,
                width: size.0,
                height: size.1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
            },
        )?;
        let color = device_image(&self.device, &self.physical, color, false)?;
        let depth = Image::new_2d(
            &self.device,
            Image2dCreateInfo {
                format: DEPTH_FORMAT,
                width: size.0,
                height: size.1,
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::SAMPLED,
            },
        )?;
        let depth = device_image(&self.device, &self.physical, depth, true)?;
        let framebuffer = Framebuffer::new(
            &self.device,
            &self.scene_pass,
            &[&color.view, &depth.view],
            size.0,
            size.1,
        )?;
        let pool = DescriptorPool::new(
            &self.device,
            1,
            &[
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::UNIFORM_BUFFER,
                    descriptor_count: 1,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: 2,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLER,
                    descriptor_count: 1,
                },
            ],
        )?;
        let set = pool.allocate(&self.post_set_layout)?;
        set.write_buffer(
            0,
            DescriptorType::UNIFORM_BUFFER,
            &self.uniforms.buffer,
            0,
            FRAME_SIZE as u64,
        );
        set.write_sampled_image(1, &color.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        set.write_sampled_image(2, &depth.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        set.write_sampler(3, &self.light_sampler);
        self.scene_target = Some(SceneTarget {
            set,
            _pool: pool,
            framebuffer,
            _color: color,
            _depth: depth,
            size,
        });
        Ok(())
    }
}
