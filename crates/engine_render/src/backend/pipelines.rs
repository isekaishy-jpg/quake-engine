//! The render passes and pipelines.

use engine_shaders::{BROKEN, Module, safe_after_depth, spirv};
use vulkane::safe::{
    AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, BufferUsage, CullMode,
    DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorType, Device, Format,
    GraphicsPipeline, GraphicsPipelineBuilder, GraphicsShaderStage, ImageLayout, InputRate,
    PipelineLayout, PushConstantRange, RenderPass, RenderPassCreateInfo, Sampler,
    SamplerCreateInfo, SamplerFilter, ShaderModule, ShaderStageFlags, VertexInputAttribute,
    VertexInputBinding,
};

use super::upload::host_buffer;
use super::{
    Backend, CanvasPass, DEPTH_FORMAT, Layouts, ParticlePipelines, Pipelines, SurfacePipelines,
};
use crate::canvas::{MAX_QUADS, UiFrame, VERTEX_FLOATS};
use crate::error::RenderError;
use crate::instances::{INSTANCE_BYTES, LOOK_BYTES, PLACE_BYTES};
use crate::sprite::PARTICLE_BYTES;
use crate::terrain::NODE_BYTES;
use crate::world::Vertex;

/// A render pass with one colour and one depth attachment.
pub(super) fn make_pass(
    device: &Device,
    format: Format,
    final_layout: ImageLayout,
) -> Result<RenderPass, RenderError> {
    Ok(RenderPass::new(
        device,
        RenderPassCreateInfo {
            color_attachments: &[AttachmentDescription {
                format,
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

/// The bytes of what the canvas is told of the screen: which it is, how bright white is, and how
/// bright the brightest is next to it.
pub(super) const CANVAS_SCREEN_BYTES: u32 = 16;

/// The canvas's pipeline, with `layout`, for `pass`.
pub(super) fn canvas_pipeline(
    device: &Device,
    layout: &PipelineLayout,
    pass: &RenderPass,
) -> Result<GraphicsPipeline, RenderError> {
    let spirv = spirv(&Module::Canvas).map_err(RenderError)?;
    let shader = ShaderModule::from_spirv(device, &spirv)?;
    let stride = u32::try_from(VERTEX_FLOATS * 4).unwrap_or(0);
    let attribute = |location, format, offset| VertexInputAttribute {
        location,
        binding: 0,
        format,
        offset,
    };
    Ok(GraphicsPipelineBuilder::new(layout, pass)
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
        .build(device)?)
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
        // What the canvas is told of the screen it's sent to.
        let layout = PipelineLayout::with_push_constants(
            &self.device,
            &[&set_layout],
            &[PushConstantRange {
                stage_flags: ShaderStageFlags::FRAGMENT,
                offset: 0,
                size: CANVAS_SCREEN_BYTES,
            }],
        )?;
        let pipeline = canvas_pipeline(&self.device, &layout, &self.present_pass)?;
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

    /// The pipeline for the fixture's models: a mesh's corners, with their own look.
    pub(super) fn make_alias_pipeline(
        device: &Device,
        layout: &PipelineLayout,
        pass: &RenderPass,
        samples: u32,
    ) -> Result<GraphicsPipeline, RenderError> {
        let spirv = spirv(&Module::Alias).map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        let (bindings, attributes) = mesh_vertices();
        GraphicsPipelineBuilder::new(layout, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_main")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_main")
            .vertex_input(&bindings, &attributes)
            .dynamic_viewport_scissor()
            // A weapon sits close enough for the near plane to open it up, and this keeps its
            // inside hidden.
            .cull_mode(CullMode::BACK)
            .samples(samples)
            .depth_test(true, true)
            .build(device)
            .map_err(RenderError::from)
    }

    /// The pipeline of sprites.
    pub(super) fn make_sprite_pipeline(
        device: &Device,
        layouts: &Layouts,
        pass: &RenderPass,
        samples: u32,
    ) -> Result<GraphicsPipeline, RenderError> {
        let spirv = spirv(&Module::Sprite).map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        Ok(GraphicsPipelineBuilder::new(&layouts.sprite, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_sprite")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_sprite")
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .samples(samples)
            .build(device)?)
    }

    /// The pipelines of particles with `code`, in their own pass.
    pub(super) fn make_particle_pipelines(
        device: &Device,
        layouts: &Layouts,
        pass: &RenderPass,
        samples: u32,
        code: &str,
    ) -> Result<ParticlePipelines, RenderError> {
        let module = Module::Particle {
            code,
            samples: samples > 1,
        };
        let spirv = spirv(&module).map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        let attribute = |location, format, offset| VertexInputAttribute {
            location,
            binding: 0,
            format,
            offset,
        };
        let bindings = [VertexInputBinding {
            binding: 0,
            stride: u32::try_from(PARTICLE_BYTES).unwrap_or(0),
            input_rate: InputRate::INSTANCE,
        }];
        let attributes = [
            attribute(0, Format::R32G32B32_SFLOAT, 0),
            attribute(1, Format::R8G8B8A8_UNORM, 12),
            attribute(2, Format::R32_SFLOAT, 16),
            attribute(3, Format::R32G32B32_SFLOAT, 20),
            attribute(4, Format::R32G32B32_SFLOAT, 32),
            attribute(5, Format::R32G32_SFLOAT, 44),
            attribute(6, Format::R32G32B32A32_SFLOAT, 52),
            attribute(7, Format::R32_SFLOAT, 68),
        ];
        let listed = GraphicsPipelineBuilder::new(&layouts.particle, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_particle")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_particle")
            .vertex_input(&bindings, &attributes)
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .samples(samples)
            .premultiplied_blending(true)
            .build(device)?;
        let worked_out = GraphicsPipelineBuilder::new(&layouts.gpu_particle, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_gpu_particle")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_particle")
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .samples(samples)
            .premultiplied_blending(true)
            .build(device)?;
        let simulated = GraphicsPipelineBuilder::new(&layouts.sim_particle, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_sim_particle")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_particle")
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .samples(samples)
            .premultiplied_blending(true)
            .build(device)?;
        Ok(ParticlePipelines {
            listed,
            worked_out,
            simulated,
        })
    }

    /// What draws surfaces with `code`, if it compiles.
    pub(super) fn make_surface_pipelines(
        device: &Device,
        layouts: &Layouts,
        pass: &RenderPass,
        samples: u32,
        code: &str,
    ) -> Result<SurfacePipelines, RenderError> {
        let spirv = spirv(&Module::Surface(code)).map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        let (bindings, attributes) = world_vertices();
        let (mesh_bindings, mesh_attributes) = mesh_vertices();
        let (land_bindings, land_attributes) = land_vertices();
        let mesh = |entry: &'static str| {
            GraphicsPipelineBuilder::new(&layouts.material, pass)
                .stage(GraphicsShaderStage::Vertex, &shader, "vs_mesh")
                .stage(GraphicsShaderStage::Fragment, &shader, entry)
                .vertex_input(&mesh_bindings, &mesh_attributes)
                .dynamic_viewport_scissor()
                .cull_mode(CullMode::NONE)
                .samples(samples)
        };
        let land = |entry: &'static str| {
            GraphicsPipelineBuilder::new(&layouts.terrain, pass)
                .stage(GraphicsShaderStage::Vertex, &shader, "vs_terrain")
                .stage(GraphicsShaderStage::Fragment, &shader, entry)
                .vertex_input(&land_bindings, &land_attributes)
                .dynamic_viewport_scissor()
                .cull_mode(CullMode::NONE)
                .samples(samples)
        };
        let surface = |entry: &'static str| {
            GraphicsPipelineBuilder::new(&layouts.material, pass)
                .stage(GraphicsShaderStage::Vertex, &shader, "vs_main")
                .stage(GraphicsShaderStage::Fragment, &shader, entry)
                .vertex_input(&bindings, &attributes)
                .dynamic_viewport_scissor()
                .cull_mode(CullMode::NONE)
                .samples(samples)
        };
        Ok(SurfacePipelines {
            code: code.to_owned(),
            after_depth_safe: safe_after_depth(code),
            opaque: surface("fs_material")
                .depth_test(true, true)
                .build(device)?,
            // What's been through the depth pass needn't write depth again.
            // Writing it would hold back the test that skips what's hidden, as
            // this shader can discard.
            after_depth: surface("fs_material")
                .depth_test(true, false)
                .build(device)?,
            // A face with holes is one side of a brush, and through them only the brush's inside
            // would show from behind, such as the underside of its top.
            masked: surface("fs_masked")
                .cull_mode(CullMode::BACK)
                .depth_test(true, true)
                .build(device)?,
            blended: surface("fs_material")
                .depth_test(true, false)
                .alpha_blending(true)
                .build(device)?,
            mesh_opaque: mesh("fs_material").depth_test(true, true).build(device)?,
            land: land("fs_land").depth_test(true, true).build(device)?,
            land_after_depth: land("fs_land").depth_test(true, false).build(device)?,
            mesh_after_depth: mesh("fs_material").depth_test(true, false).build(device)?,
            mesh_masked: mesh("fs_masked").depth_test(true, true).build(device)?,
            mesh_blended: mesh("fs_material")
                .depth_test(true, false)
                .alpha_blending(true)
                .build(device)?,
        })
    }

    /// The pipelines, with the standard surface as the only one so far.
    pub(super) fn make_pipelines(
        device: &Device,
        layouts: &Layouts,
        pass: &RenderPass,
        samples: u32,
    ) -> Result<Pipelines, RenderError> {
        let surface_spirv = spirv(&Module::Surface("")).map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &surface_spirv)?;
        let (bindings, attributes) = world_vertices();
        let sprite = Self::make_sprite_pipeline(device, layouts, pass, samples)?;
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
        let plain_spirv = spirv(&Module::PlainSurface).map_err(RenderError)?;
        let plain_shader = ShaderModule::from_spirv(device, &plain_spirv)?;
        let plain_after_depth = GraphicsPipelineBuilder::new(&layouts.material, pass)
            .stage(GraphicsShaderStage::Vertex, &plain_shader, "vs_main")
            .stage(GraphicsShaderStage::Fragment, &plain_shader, "fs_material")
            .vertex_input(&bindings, &attributes)
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, false)
            .samples(samples)
            .build(device)?;
        let (mesh_bindings, mesh_attributes) = mesh_vertices();
        let mesh_hull = GraphicsPipelineBuilder::new(&layouts.material, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_mesh_hull")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_mesh_hull")
            .vertex_input(&mesh_bindings, &mesh_attributes)
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::FRONT)
            .depth_test(true, true)
            .samples(samples)
            .build(device)?;
        let mesh_depth = GraphicsPipelineBuilder::new(&layouts.material, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_mesh")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_depth")
            .vertex_input(&mesh_bindings, &mesh_attributes)
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .color_writes(false)
            .samples(samples)
            .build(device)?;
        let (land_bindings, land_attributes) = land_vertices();
        let land_depth = GraphicsPipelineBuilder::new(&layouts.terrain, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_terrain")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_land_depth")
            .vertex_input(&land_bindings, &land_attributes)
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .color_writes(false)
            .samples(samples)
            .build(device)?;
        Ok(Pipelines {
            mesh_hull,
            mesh_depth,
            land_depth,
            depth_only,
            plain_after_depth,
            sky_sphere: GraphicsPipelineBuilder::new(&layouts.material, pass)
                .stage(GraphicsShaderStage::Vertex, &shader, "vs_sky_sphere")
                .stage(GraphicsShaderStage::Fragment, &shader, "fs_sky_sphere")
                .dynamic_viewport_scissor()
                .cull_mode(CullMode::NONE)
                .depth_test(false, false)
                .samples(samples)
                .build(device)?,
            sky_window: GraphicsPipelineBuilder::new(&layouts.material, pass)
                .stage(GraphicsShaderStage::Vertex, &shader, "vs_main")
                .stage(GraphicsShaderStage::Fragment, &shader, "fs_sky_window")
                .vertex_input(&bindings, &attributes)
                .dynamic_viewport_scissor()
                .cull_mode(CullMode::NONE)
                .depth_test(true, true)
                .samples(samples)
                .build(device)?,
            surfaces: vec![Self::make_surface_pipelines(
                device, layouts, pass, samples, "",
            )?],
            alias: Self::make_alias_pipeline(device, &layouts.alias, pass, samples)?,
            sprite,
        })
    }

    /// The place of the shader with `code`, making its pipelines if it's new. If
    /// it won't compile, that's noted, and it shows green.
    pub(super) fn surface_for(&mut self, name: &str, code: &str) -> Result<usize, RenderError> {
        let code = if code.trim().is_empty() && !name.is_empty() {
            self.shader_problems
                .push(format!("{name}: there's no such shader"));
            BROKEN
        } else {
            code
        };
        let known = |surfaces: &[SurfacePipelines], code: &str| {
            surfaces.iter().position(|surface| surface.code == code)
        };
        if let Some(found) = known(&self.pipelines.surfaces, code) {
            return Ok(found);
        }
        let made = Self::make_surface_pipelines(
            &self.device,
            &self.layouts,
            &self.scene_pass,
            self.samples,
            code,
        );
        let surface = match made {
            Ok(surface) => surface,
            Err(error) => {
                self.shader_problems.push(format!("{name}: {error}"));
                if let Some(found) = known(&self.pipelines.surfaces, BROKEN) {
                    return Ok(found);
                }
                Self::make_surface_pipelines(
                    &self.device,
                    &self.layouts,
                    &self.scene_pass,
                    self.samples,
                    BROKEN,
                )?
            }
        };
        self.pipelines.surfaces.push(surface);
        Ok(self.pipelines.surfaces.len() - 1)
    }
}

/// How world vertices are laid out.
/// What a copy of a shared mesh reads: its corners, as the world's, and then the copy.
fn mesh_vertices() -> ([VertexInputBinding; 3], [VertexInputAttribute; 9]) {
    let bindings = [
        VertexInputBinding {
            binding: 0,
            stride: u32::try_from(PLACE_BYTES).unwrap_or(0),
            input_rate: InputRate::VERTEX,
        },
        VertexInputBinding {
            binding: 1,
            stride: u32::try_from(LOOK_BYTES).unwrap_or(0),
            input_rate: InputRate::VERTEX,
        },
        VertexInputBinding {
            binding: 2,
            stride: u32::try_from(INSTANCE_BYTES).unwrap_or(0),
            input_rate: InputRate::INSTANCE,
        },
    ];
    let attribute = |location, binding, format, offset| VertexInputAttribute {
        location,
        binding,
        format,
        offset,
    };
    let attributes = [
        attribute(0, 0, Format::R32G32B32_SFLOAT, 0),
        attribute(1, 1, Format::R32G32_SFLOAT, 0),
        attribute(11, 1, Format::R8G8B8A8_UNORM, 8),
        attribute(5, 0, Format::R8G8B8A8_SNORM, 12),
        attribute(6, 2, Format::R32G32B32A32_SFLOAT, 0),
        attribute(7, 2, Format::R32G32B32A32_SFLOAT, 16),
        attribute(8, 2, Format::R32G32B32A32_SFLOAT, 32),
        attribute(9, 2, Format::R32G32B32_SFLOAT, 48),
        attribute(10, 2, Format::R8G8B8A8_UINT, 60),
    ];
    (bindings, attributes)
}

/// What a part of the land reads: only its node, as its corners come from the heights.
pub(super) fn land_vertices() -> ([VertexInputBinding; 1], [VertexInputAttribute; 4]) {
    let bindings = [VertexInputBinding {
        binding: 0,
        stride: u32::try_from(NODE_BYTES).unwrap_or(0),
        input_rate: InputRate::INSTANCE,
    }];
    let attribute = |location, format, offset| VertexInputAttribute {
        location,
        binding: 0,
        format,
        offset,
    };
    let attributes = [
        attribute(0, Format::R32G32B32A32_UINT, 0),
        attribute(1, Format::R32G32B32A32_SFLOAT, 16),
        attribute(2, Format::R32G32B32A32_SFLOAT, 32),
        attribute(3, Format::R32G32B32A32_UINT, 48),
    ];
    (bindings, attributes)
}

fn world_vertices() -> ([VertexInputBinding; 1], [VertexInputAttribute; 7]) {
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
        attribute(5, Format::R8G8B8A8_SNORM, 36),
        attribute(11, Format::R8G8B8A8_UNORM, 40),
    ];
    (bindings, attributes)
}
