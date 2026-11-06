//! The render passes and pipelines.

use vulkane::safe::{
    AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, BufferUsage, CullMode,
    DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorType, Device, Format, FrontFace,
    GraphicsPipeline, GraphicsPipelineBuilder, GraphicsShaderStage, ImageLayout, InputRate,
    PipelineLayout, RenderPass, RenderPassCreateInfo, Sampler, SamplerCreateInfo, SamplerFilter,
    ShaderModule, ShaderStageFlags, VertexInputAttribute, VertexInputBinding,
};

use super::upload::host_buffer;
use super::{
    ALIAS_SHADER, Backend, CANVAS_SHADER, COLOR_FORMAT, CanvasPass, DEPTH_FORMAT, Layouts,
    Pipelines, SPRITE_SHADER, WORLD_SHADER,
};
use crate::alias::POSE_CORNER_BYTES;
use crate::canvas::{MAX_QUADS, UiFrame, VERTEX_FLOATS};
use crate::error::RenderError;
use crate::sprite::PARTICLE_BYTES;
use crate::world::Vertex;

/// A render pass with one colour and one depth attachment.
pub(super) fn make_pass(
    device: &Device,
    final_layout: ImageLayout,
) -> Result<RenderPass, RenderError> {
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

impl Backend {
    /// Sets up the canvas's pipeline.
    pub(super) fn make_canvas(&self) -> Result<CanvasPass, RenderError> {
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
        let layout = PipelineLayout::new(&self.device, &[&set_layout])?;
        let spirv = vulkane::safe::naga::compile_wgsl(CANVAS_SHADER)
            .map_err(|error| RenderError(error.to_string()))?;
        let shader = ShaderModule::from_spirv(&self.device, &spirv)?;
        let stride = u32::try_from(VERTEX_FLOATS * 4).unwrap_or(0);
        let attribute = |location, format, offset| VertexInputAttribute {
            location,
            binding: 0,
            format,
            offset,
        };
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
                    attribute(0, Format::R32G32_SFLOAT, 0),
                    attribute(1, Format::R32G32_SFLOAT, 8),
                    attribute(2, Format::R32G32B32A32_SFLOAT, 16),
                ],
            )
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(false, false)
            .alpha_blending(true)
            .build(&self.device)?;
        let buffer = || {
            host_buffer(
                &self.device,
                &self.physical,
                MAX_QUADS * 6 * VERTEX_FLOATS * 4,
                BufferUsage::VERTEX_BUFFER,
            )
        };
        let vertices = [buffer()?, buffer()?];
        Ok(CanvasPass {
            pipeline,
            layout,
            set_layout,
            sampler,
            pictures: Vec::new(),
            vertices,
            slot: 0,
            batches: Vec::new(),
            ui: UiFrame::default(),
            ui_first: 0,
        })
    }

    /// The pipeline for models. Each pose is a vertex buffer at its own offset,
    /// bound twice, so the shader can blend two of them.
    pub(super) fn make_alias_pipeline(
        device: &Device,
        layout: &PipelineLayout,
        pass: &RenderPass,
        entries: (&str, &str),
        cull: CullMode,
        samples: u32,
    ) -> Result<GraphicsPipeline, RenderError> {
        let spirv = vulkane::safe::naga::compile_wgsl(ALIAS_SHADER)
            .map_err(|error| RenderError(error.to_string()))?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        let pose_stride = u32::try_from(POSE_CORNER_BYTES).unwrap_or(0);
        let binding = |binding, stride| VertexInputBinding {
            binding,
            stride,
            input_rate: InputRate::VERTEX,
        };
        let attribute = |location, binding, format, offset| VertexInputAttribute {
            location,
            binding,
            format,
            offset,
        };
        GraphicsPipelineBuilder::new(layout, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, entries.0)
            .stage(GraphicsShaderStage::Fragment, &shader, entries.1)
            .vertex_input(
                &[
                    binding(0, pose_stride),
                    binding(1, pose_stride),
                    binding(2, 8),
                ],
                &[
                    attribute(0, 0, Format::R8G8B8A8_UINT, 0),
                    attribute(1, 0, Format::R8G8B8A8_SNORM, 4),
                    attribute(2, 1, Format::R8G8B8A8_UINT, 0),
                    attribute(3, 1, Format::R8G8B8A8_SNORM, 4),
                    attribute(4, 2, Format::R32G32_SFLOAT, 0),
                ],
            )
            .dynamic_viewport_scissor()
            // Quake's triangles wind clockwise. A weapon sits close enough for
            // the near plane to open it up, and this keeps its inside hidden.
            .cull_mode(cull)
            .samples(samples)
            .front_face(FrontFace::CLOCKWISE)
            .depth_test(true, true)
            .build(device)
            .map_err(RenderError::from)
    }

    /// The pipelines of sprites and particles.
    pub(super) fn make_effect_pipelines(
        device: &Device,
        layouts: &Layouts,
        pass: &RenderPass,
        samples: u32,
    ) -> Result<(GraphicsPipeline, GraphicsPipeline), RenderError> {
        let spirv = vulkane::safe::naga::compile_wgsl(SPRITE_SHADER)
            .map_err(|error| RenderError(error.to_string()))?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        let sprite = GraphicsPipelineBuilder::new(&layouts.sprite, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_sprite")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_sprite")
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .samples(samples)
            .build(device)?;
        let particle = GraphicsPipelineBuilder::new(&layouts.particle, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_particle")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_particle")
            .vertex_input(
                &[VertexInputBinding {
                    binding: 0,
                    stride: u32::try_from(PARTICLE_BYTES).unwrap_or(0),
                    input_rate: InputRate::INSTANCE,
                }],
                &[
                    VertexInputAttribute {
                        location: 0,
                        binding: 0,
                        format: Format::R32G32B32_SFLOAT,
                        offset: 0,
                    },
                    VertexInputAttribute {
                        location: 1,
                        binding: 0,
                        format: Format::R8G8B8A8_UNORM,
                        offset: 12,
                    },
                ],
            )
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, false)
            .samples(samples)
            .build(device)?;
        Ok((sprite, particle))
    }

    /// The pipelines, most of which differ only in their fragment stage.
    pub(super) fn make_pipelines(
        device: &Device,
        layouts: &Layouts,
        pass: &RenderPass,
        samples: u32,
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
            GraphicsPipelineBuilder::new(&layouts.world, pass)
                .stage(GraphicsShaderStage::Vertex, &shader, "vs_main")
                .stage(GraphicsShaderStage::Fragment, &shader, entry)
                .vertex_input(&bindings, &attributes)
                .dynamic_viewport_scissor()
                .cull_mode(CullMode::NONE)
                .depth_test(true, true)
                .samples(samples)
                .build(device)
        };
        let material = |blended: bool, entry: &str| {
            GraphicsPipelineBuilder::new(&layouts.material, pass)
                .stage(GraphicsShaderStage::Vertex, &shader, "vs_main")
                .stage(GraphicsShaderStage::Fragment, &shader, entry)
                .vertex_input(&bindings, &attributes)
                .dynamic_viewport_scissor()
                .cull_mode(CullMode::NONE)
                .depth_test(true, !blended)
                .alpha_blending(blended)
                .samples(samples)
                .build(device)
        };
        let (sprite, particle) = Self::make_effect_pipelines(device, layouts, pass, samples)?;
        let depth_only = GraphicsPipelineBuilder::new(&layouts.world, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_main")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_depth")
            .vertex_input(&bindings, &attributes)
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .color_writes(false)
            .samples(samples)
            .build(device)?;
        // What's been through the depth pass needn't write depth again. Writing
        // it would hold back the test that skips what's hidden, as this shader
        // can discard.
        let after_depth = |shader: &ShaderModule| {
            GraphicsPipelineBuilder::new(&layouts.material, pass)
                .stage(GraphicsShaderStage::Vertex, shader, "vs_main")
                .stage(GraphicsShaderStage::Fragment, shader, "fs_material")
                .vertex_input(&bindings, &attributes)
                .dynamic_viewport_scissor()
                .cull_mode(CullMode::NONE)
                .depth_test(true, false)
                .samples(samples)
                .build(device)
        };
        let material_after_depth = after_depth(&shader)?;
        let plain_source =
            WORLD_SHADER.replace("const PLAIN: bool = false;", "const PLAIN: bool = true;");
        let plain_spirv = vulkane::safe::naga::compile_wgsl(&plain_source)
            .map_err(|error| RenderError(error.to_string()))?;
        let plain_shader = ShaderModule::from_spirv(device, &plain_spirv)?;
        let plain_after_depth = after_depth(&plain_shader)?;
        Ok(Pipelines {
            depth_only,
            material_after_depth,
            plain_after_depth,
            solid: pipeline("fs_solid")?,
            cutout: pipeline("fs_cutout")?,
            liquid: pipeline("fs_liquid")?,
            masked: material(false, "fs_masked")?,
            sky: pipeline("fs_sky")?,
            sky_sphere: GraphicsPipelineBuilder::new(&layouts.world, pass)
                .stage(GraphicsShaderStage::Vertex, &shader, "vs_sky_sphere")
                .stage(GraphicsShaderStage::Fragment, &shader, "fs_sky_sphere")
                .dynamic_viewport_scissor()
                .cull_mode(CullMode::NONE)
                .depth_test(false, false)
                .samples(samples)
                .build(device)?,
            sky_window: pipeline("fs_sky_window")?,
            material: material(false, "fs_material")?,
            blended: material(true, "fs_material")?,
            alias: Self::make_alias_pipeline(
                device,
                &layouts.alias,
                pass,
                ("vs_main", "fs_main"),
                CullMode::BACK,
                samples,
            )?,
            alias_hull: Self::make_alias_pipeline(
                device,
                &layouts.alias,
                pass,
                ("vs_hull", "fs_hull"),
                CullMode::FRONT,
                samples,
            )?,
            sprite,
            particle,
        })
    }
}
