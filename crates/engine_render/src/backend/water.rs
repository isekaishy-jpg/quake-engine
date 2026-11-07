//! The surfaces of shaped bodies of water, drawn into a picture of their own for the final pass to
//! read (proposal 0147).

use engine_core::math::Vec3;
use engine_map::{Map, Pool, SlotImage, WaterSheet};
use engine_shaders::{Module, spirv};
use vulkane::safe::{
    AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, BufferUsage, ClearValue, CullMode,
    DescriptorSet, DescriptorSetLayout, Device, Format, Framebuffer, GraphicsPipeline,
    GraphicsPipelineBuilder, GraphicsShaderStage, Image, Image2dCreateInfo, ImageLayout,
    ImageUsage, InputRate, PipelineLayout, PushConstantRange, RenderPass, RenderPassCreateInfo,
    ShaderModule, ShaderStageFlags, VertexInputAttribute, VertexInputBinding,
};

use super::upload::{device_image, nearest_bodies};
use super::{Backend, DEPTH_FORMAT, GpuBuffer, GpuImage};
use crate::error::RenderError;

/// What the picture holds: how far, which body, and how it slopes. A half float can't tell how far
/// to a unit, which is what a shore goes by.
const WATER_FORMAT: Format = Format::R32G32B32A32_SFLOAT;

/// Bytes of what a draw pushes.
const WATER_PUSH: u32 = 16;

/// Bytes of a corner.
const CORNER_BYTES: u32 = 12;

/// The pass surfaces are drawn in, which starts with nothing and sorts them by a depth of its own.
pub(super) fn make_water_pass(device: &Device) -> Result<RenderPass, RenderError> {
    Ok(RenderPass::new(
        device,
        RenderPassCreateInfo {
            color_attachments: &[AttachmentDescription {
                format: WATER_FORMAT,
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

/// The layout of surfaces' draws.
pub(super) fn make_water_layout(
    device: &Device,
    frame: &DescriptorSetLayout,
) -> Result<PipelineLayout, RenderError> {
    Ok(PipelineLayout::with_push_constants(
        device,
        &[frame],
        &[PushConstantRange {
            stage_flags: ShaderStageFlags::FRAGMENT,
            offset: 0,
            size: WATER_PUSH,
        }],
    )?)
}

/// How surfaces are drawn.
pub(super) fn make_water_pipeline(
    device: &Device,
    layout: &PipelineLayout,
    pass: &RenderPass,
) -> Result<GraphicsPipeline, RenderError> {
    let words = spirv(&Module::Water).map_err(RenderError)?;
    let shader = ShaderModule::from_spirv(device, &words)?;
    let bindings = [VertexInputBinding {
        binding: 0,
        stride: CORNER_BYTES,
        input_rate: InputRate::VERTEX,
    }];
    let attributes = [VertexInputAttribute {
        location: 0,
        binding: 0,
        format: Format::R32G32B32_SFLOAT,
        offset: 0,
    }];
    Ok(GraphicsPipelineBuilder::new(layout, pass)
        .stage(GraphicsShaderStage::Vertex, &shader, "vs_water")
        .stage(GraphicsShaderStage::Fragment, &shader, "fs_water")
        .vertex_input(&bindings, &attributes)
        .dynamic_viewport_scissor()
        .cull_mode(CullMode::NONE)
        .depth_test(true, true)
        .build(device)?)
}

/// The picture and what's drawn into it, at a scene's size.
pub(super) struct WaterTarget {
    /// What the final pass reads.
    pub(super) image: GpuImage,
    /// Kept alive for the framebuffer.
    _depth: GpuImage,
    /// What's drawn into.
    pub(super) framebuffer: Framebuffer,
}

/// The surfaces of a level on the GPU.
pub(super) struct GpuWater {
    /// Each sheet's corners, three to a triangle, and how many.
    sheets: Vec<(GpuBuffer, u32)>,
    /// What's drawn this frame: which sheet, and which of the frame's bodies it is, plus one.
    shown: Vec<(usize, f32)>,
}

impl Backend {
    /// Makes the picture of surfaces for a scene of `size`.
    pub(super) fn water_target(&self, size: (u32, u32)) -> Result<WaterTarget, RenderError> {
        let image = Image::new_2d(
            &self.device,
            Image2dCreateInfo {
                format: WATER_FORMAT,
                width: size.0,
                height: size.1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
            },
        )?;
        let image = device_image(&self.device, &self.physical, image, false)?;
        let depth = Image::new_2d(
            &self.device,
            Image2dCreateInfo {
                format: DEPTH_FORMAT,
                width: size.0,
                height: size.1,
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
            },
        )?;
        let depth = device_image(&self.device, &self.physical, depth, true)?;
        let framebuffer = Framebuffer::new(
            &self.device,
            &self.water_pass,
            &[&image.view, &depth.view],
            size.0,
            size.1,
        )?;
        Ok(WaterTarget {
            image,
            _depth: depth,
            framebuffer,
        })
    }

    /// Puts what `map` has over its water on the GPU, and has the final pass read it.
    pub(super) fn upload_water_pictures(&mut self, map: &Map) -> Result<(), RenderError> {
        let pictures: Vec<&SlotImage> = map.water_pictures.iter().collect();
        self.water_pictures = Some(self.upload_array(&pictures, Format::R8G8B8A8_SRGB)?);
        if let Some(target) = &self.scene_target {
            self.write_water_pictures(&target.set);
        }
        Ok(())
    }

    /// Points `set` at what's over water, if that's there yet.
    pub(super) fn write_water_pictures(&self, set: &DescriptorSet) {
        if let Some(image) = &self.water_pictures {
            set.write_sampled_image(12, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        }
    }

    /// Puts `map`'s surfaces on the GPU, if it has any.
    pub(super) fn upload_water(&self, map: &Map) -> Result<Option<GpuWater>, RenderError> {
        if map.water_sheets.is_empty() {
            return Ok(None);
        }
        let mut sheets = Vec::with_capacity(map.water_sheets.len());
        for sheet in &map.water_sheets {
            let corners = sheet.triangles();
            let mut bytes = Vec::with_capacity(corners.len() * 12);
            for corner in &corners {
                for part in [corner.x, corner.y, corner.z] {
                    bytes.extend_from_slice(&part.to_le_bytes());
                }
            }
            bytes.resize(bytes.len().max(12), 0);
            let buffer = self.device_buffer(&bytes, BufferUsage::VERTEX_BUFFER)?;
            sheets.push((buffer, u32::try_from(corners.len()).unwrap_or(0)));
        }
        Ok(Some(GpuWater {
            sheets,
            shown: Vec::new(),
        }))
    }

    /// Works out which surfaces this frame draws, as the frame's bodies have them.
    pub(super) fn plan_water(&mut self, pools: &[Pool], sheets: &[WaterSheet], eye: Vec3) {
        let Some(gpu) = self.map.as_mut().and_then(|map| map.water.as_mut()) else {
            return;
        };
        gpu.shown.clear();
        for (slot, (_, sheet)) in (1_u16..).zip(nearest_bodies(pools, sheets, eye)) {
            if let Some(sheet) = sheet {
                gpu.shown.push((sheet, f32::from(slot)));
            }
        }
    }

    /// Draws this frame's surfaces into the picture.
    pub(super) fn record_water(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        target: &super::post::SceneTarget,
    ) {
        let Some(map) = &self.map else {
            return;
        };
        let Some(gpu) = &map.water else {
            return;
        };
        let clear = [
            ClearValue::Color([0.0; 4]),
            ClearValue::DepthStencil {
                depth: 1.0,
                stencil: 0,
            },
        ];
        rec.begin_render_pass_ext(&self.water_pass, &target.water.framebuffer, &clear);
        let size = target.size;
        #[expect(
            clippy::cast_precision_loss,
            reason = "window sizes are far below 2^24"
        )]
        rec.set_viewport(0.0, 0.0, size.0 as f32, size.1 as f32);
        rec.set_scissor(0, 0, size.0, size.1);
        rec.bind_graphics_pipeline(&self.water_pipeline);
        rec.bind_graphics_descriptor_sets(&self.water_layout, 0, &[&map.frame_set]);
        for (sheet, slot) in &gpu.shown {
            let Some((buffer, count)) = gpu.sheets.get(*sheet) else {
                continue;
            };
            let mut push = [0; WATER_PUSH as usize];
            push[..4].copy_from_slice(&slot.to_le_bytes());
            rec.push_constants(&self.water_layout, ShaderStageFlags::FRAGMENT, 0, &push);
            rec.bind_vertex_buffers(0, &[(&buffer.buffer, 0)]);
            rec.draw(*count, 1, 0, 0);
        }
        rec.end_render_pass();
    }
}
