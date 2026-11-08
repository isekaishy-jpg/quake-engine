//! Which way what moves went since the last frame, for TAA (proposal 0151). Models, moving meshes
//! and brush models that moved are drawn again into an image, with where each was last frame. The
//! world that doesn't move is found from depth and where the view was.

use std::collections::HashMap;

use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_shaders::{Module, spirv};
use vulkane::safe::{
    AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, ClearValue, CullMode,
    DescriptorSetLayout, Device, Format, Framebuffer, GraphicsPipeline, GraphicsPipelineBuilder,
    GraphicsShaderStage, Image, Image2dCreateInfo, ImageLayout, ImageUsage, InputRate,
    PhysicalDevice, PipelineLayout, PushConstantRange, RenderPass, RenderPassCreateInfo,
    ShaderModule, ShaderStageFlags, VertexInputAttribute, VertexInputBinding,
};

use super::deform::{bind_figure, bind_moving};
use super::record::Recorder;
use super::upload::{depth_image, device_image};
use super::{Backend, DEPTH_FORMAT, GpuImage, INDEX_TYPE_U32, Scene};
use crate::error::RenderError;
use crate::instances::{INSTANCE_BYTES, PLACE_BYTES};
use crate::world::Vertex;

/// What's kept of where something went: across the screen, how far from the eye, and whether
/// anything's there.
const MOTION_FORMAT: Format = Format::R16G16B16A16_SFLOAT;

/// The bytes of a copy's rows last frame.
pub(super) const LAST_ROWS_BYTES: usize = 48;

/// The bytes of where a brush model is now and was.
const BRUSH_PLACES_BYTES: u32 = 32;

/// The render pass of what moved: its own depth, so that the nearest of what moves is kept.
pub(super) fn make_motion_pass(device: &Device) -> Result<RenderPass, RenderError> {
    Ok(RenderPass::new(
        device,
        RenderPassCreateInfo {
            color_attachments: &[AttachmentDescription {
                format: MOTION_FORMAT,
                load_op: AttachmentLoadOp::CLEAR,
                store_op: AttachmentStoreOp::STORE,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
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

/// Where what moved is drawn, for a scene of one size.
pub(super) struct MotionTarget {
    /// Where it's drawn.
    framebuffer: Framebuffer,
    /// What's drawn.
    pub(super) image: GpuImage,
    /// Kept alive for the framebuffer.
    _depth: GpuImage,
    /// Its size.
    size: (u32, u32),
}

impl MotionTarget {
    /// Makes one of `size`, drawn with `pass`.
    pub(super) fn new(
        device: &Device,
        physical: &PhysicalDevice,
        pass: &RenderPass,
        size: (u32, u32),
    ) -> Result<Self, RenderError> {
        let image = Image::new_2d(
            device,
            Image2dCreateInfo {
                format: MOTION_FORMAT,
                width: size.0,
                height: size.1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
            },
        )?;
        let image = device_image(device, physical, image, false)?;
        let depth = depth_image(device, physical, size)?;
        let framebuffer =
            Framebuffer::new(device, pass, &[&image.view, &depth.view], size.0, size.1)?;
        Ok(Self {
            framebuffer,
            image,
            _depth: depth,
            size,
        })
    }
}

/// What draws what moved.
pub(super) struct MotionPipelines {
    /// Models and moving meshes, which have rows.
    mesh: GraphicsPipeline,
    /// Brush models, which have a place.
    brush: GraphicsPipeline,
    /// Their layout: the frame, and where a brush model is and was.
    layout: PipelineLayout,
}

impl MotionPipelines {
    /// Makes them, for `pass`, with the frame's set laid out as `frame_layout`.
    pub(super) fn new(
        device: &Device,
        frame_layout: &DescriptorSetLayout,
        pass: &RenderPass,
    ) -> Result<Self, RenderError> {
        let layout = PipelineLayout::with_push_constants(
            device,
            &[frame_layout],
            &[PushConstantRange {
                stage_flags: ShaderStageFlags::VERTEX,
                offset: 0,
                size: BRUSH_PLACES_BYTES,
            }],
        )?;
        let spirv = spirv(&Module::MotionVectors).map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        let attribute = |location, binding, format, offset| VertexInputAttribute {
            location,
            binding,
            format,
            offset,
        };
        let stride = |bytes: usize| u32::try_from(bytes).unwrap_or(0);
        let rows = Format::R32G32B32A32_SFLOAT;
        let mesh = GraphicsPipelineBuilder::new(&layout, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_mesh")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_motion")
            .vertex_input(
                &[
                    VertexInputBinding {
                        binding: 0,
                        stride: stride(PLACE_BYTES),
                        input_rate: InputRate::VERTEX,
                    },
                    VertexInputBinding {
                        binding: 2,
                        stride: stride(INSTANCE_BYTES),
                        input_rate: InputRate::INSTANCE,
                    },
                    VertexInputBinding {
                        binding: 3,
                        stride: stride(LAST_ROWS_BYTES),
                        input_rate: InputRate::INSTANCE,
                    },
                ],
                &[
                    attribute(0, 0, Format::R32G32B32_SFLOAT, 0),
                    attribute(6, 2, rows, 0),
                    attribute(7, 2, rows, 16),
                    attribute(8, 2, rows, 32),
                    attribute(9, 2, Format::R32G32B32_SFLOAT, 48),
                    attribute(12, 3, rows, 0),
                    attribute(13, 3, rows, 16),
                    attribute(14, 3, rows, 32),
                ],
            )
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .build(device)?;
        let brush = GraphicsPipelineBuilder::new(&layout, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_brush")
            .stage(GraphicsShaderStage::Fragment, &shader, "fs_motion")
            .vertex_input(
                &[VertexInputBinding {
                    binding: 0,
                    stride: stride(Vertex::SIZE),
                    input_rate: InputRate::VERTEX,
                }],
                &[attribute(0, 0, Format::R32G32B32_SFLOAT, 0)],
            )
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(true, true)
            .build(device)?;
        Ok(Self {
            mesh,
            brush,
            layout,
        })
    }
}

/// A brush model that moved: which, where it is, and where it was.
pub(super) type MovedBrush = (u32, Vec3, Vec3);

/// The brush models in `instances` that are drawn and moved since `last`, which becomes where
/// each is now.
pub(super) fn moved_brushes(
    instances: &[ModelInstance],
    last: &mut HashMap<u32, Vec3>,
) -> Vec<MovedBrush> {
    let mut moved = Vec::new();
    let mut now = HashMap::with_capacity(instances.len());
    for instance in instances {
        let before = last.get(&instance.model).copied();
        if let Some(before) = before.filter(|before| *before != instance.origin)
            && instance.visible
        {
            moved.push((instance.model, instance.origin, before));
        }
        now.insert(instance.model, instance.origin);
    }
    *last = now;
    moved
}

impl Backend {
    /// Notes which brush models moved since the last frame, for TAA.
    pub(super) fn note_moved_brushes(&mut self, scene: Option<&Scene<'_>>) {
        let instances = scene.map_or(&[][..], |scene| scene.drawables.instances);
        self.moved_brushes = moved_brushes(instances, &mut self.last_brush_origins);
    }
}

/// Where a brush model is now and was, or with `now.w` 1 for models, as push constant bytes.
fn places_bytes(now: [f32; 4], before: [f32; 4]) -> [u8; BRUSH_PLACES_BYTES as usize] {
    let mut told = [0_u8; BRUSH_PLACES_BYTES as usize];
    let values = now.into_iter().chain(before);
    for (chunk, value) in told.as_chunks_mut::<4>().0.iter_mut().zip(values) {
        *chunk = value.to_le_bytes();
    }
    told
}

impl Recorder<'_> {
    /// Records what moved into `target`: models, moving meshes, and `brushes`.
    pub(super) fn record_motion(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        (pass, target, pipelines): (&RenderPass, &MotionTarget, &MotionPipelines),
        scene: &Scene<'_>,
        brushes: &[MovedBrush],
    ) {
        rec.begin_render_pass_ext(
            pass,
            &target.framebuffer,
            &[
                ClearValue::Color([0.0; 4]),
                ClearValue::DepthStencil {
                    depth: 1.0,
                    stencil: 0,
                },
            ],
        );
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        rec.set_viewport(0.0, 0.0, target.size.0 as f32, target.size.1 as f32);
        rec.set_scissor(0, 0, target.size.0, target.size.1);
        let map = self.map;
        let moving = &map.moving;
        let last = &moving.last_rows(self.slot).buffer;
        rec.bind_graphics_pipeline(&pipelines.mesh);
        rec.bind_graphics_descriptor_sets(&pipelines.layout, 0, &[&map.frame_set]);
        let vertex = ShaderStageFlags::VERTEX;
        rec.push_constants(
            &pipelines.layout,
            vertex,
            0,
            &places_bytes([0.0; 4], [0.0; 4]),
        );
        for job in moving.jobs(self.slot).iter().filter(|job| job.in_view) {
            let Some(mesh) = moving.meshes.get(job.mesh as usize) else {
                continue;
            };
            if bind_moving(rec, moving, job, self.slot, true).is_none() {
                continue;
            }
            rec.bind_vertex_buffers(3, &[(last, 0)]);
            for part in mesh.parts(job.level) {
                rec.draw_indexed(part.count, 1, part.first, 0, job.instance);
            }
        }
        let models = [0.0, 0.0, 0.0, 1.0];
        rec.push_constants(
            &pipelines.layout,
            vertex,
            0,
            &places_bytes(models, [0.0; 4]),
        );
        for job in moving.figures(self.slot) {
            let Some(gpu) = self.models.get(job.model as usize) else {
                continue;
            };
            bind_figure(rec, moving, gpu, job.moved_to, self.slot);
            rec.bind_vertex_buffers(3, &[(last, 0)]);
            rec.draw_indexed(gpu.index_count, 1, 0, 0, job.instance);
        }
        if !brushes.is_empty() {
            rec.bind_graphics_pipeline(&pipelines.brush);
            rec.bind_vertex_buffers(0, &[(&map.vertices.buffer, 0)]);
            rec.bind_index_buffer(&self.indices.buffer, 0, INDEX_TYPE_U32);
            for &(model, now, before) in brushes {
                let Some(ranges) = scene.mesh.model_ranges.get(model as usize) else {
                    continue;
                };
                let told = places_bytes(
                    [now.x, now.y, now.z, 0.0],
                    [before.x, before.y, before.z, 0.0],
                );
                rec.push_constants(&pipelines.layout, vertex, 0, &told);
                for range in ranges {
                    let first = scene.mesh.world_index_count + range.first;
                    rec.draw_indexed(range.count, 1, first, 0, 0);
                }
            }
        }
        rec.end_render_pass();
    }
}
