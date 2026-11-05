//! Drawing the scene off-screen, so a final pass can read its colour and depth
//! (proposal 0075).

use vulkane::safe::{
    AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, CullMode, DescriptorPool,
    DescriptorPoolSize, DescriptorSet, DescriptorType, Device, Format, Framebuffer,
    GraphicsPipeline, GraphicsPipelineBuilder, GraphicsShaderStage, Image, Image2dCreateInfo,
    ImageLayout, ImageUsage, PipelineLayout, RenderPass, RenderPassCreateInfo, ShaderModule,
};

use engine_map::{Map, ProbeGrid};

use super::smaa::SmaaTarget;
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
    /// Kept alive for the framebuffer, with several samples a pixel.
    _resolved: Option<GpuImage>,
    /// What SMAA draws.
    pub(super) smaa: SmaaTarget,
    /// Its size.
    pub(super) size: (u32, u32),
}

/// The render pass of a scene, leaving what the final pass reads ready to
/// read. With several samples a pixel, colour resolves to an attachment after
/// the others, and depth stays as it is.
pub(super) fn make_scene_pass(device: &Device, samples: u32) -> Result<RenderPass, RenderError> {
    let resolved = AttachmentDescription {
        format: COLOR_FORMAT,
        load_op: AttachmentLoadOp::DONT_CARE,
        store_op: AttachmentStoreOp::STORE,
        initial_layout: ImageLayout::UNDEFINED,
        final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    };
    let (color_end, resolve) = if samples > 1 {
        (ImageLayout::COLOR_ATTACHMENT_OPTIMAL, Some(resolved))
    } else {
        (ImageLayout::SHADER_READ_ONLY_OPTIMAL, None)
    };
    Ok(RenderPass::new_multisampled(
        device,
        RenderPassCreateInfo {
            color_attachments: &[AttachmentDescription {
                format: COLOR_FORMAT,
                load_op: AttachmentLoadOp::CLEAR,
                store_op: AttachmentStoreOp::STORE,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout: color_end,
            }],
            depth_attachment: Some(AttachmentDescription {
                format: DEPTH_FORMAT,
                load_op: AttachmentLoadOp::CLEAR,
                store_op: AttachmentStoreOp::STORE,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }),
        },
        samples,
        resolve,
    )?)
}

/// The probes as an image: each level of them is a slice, stacked downwards,
/// and each probe the average of its sides.
fn probe_slices(probes: &ProbeGrid) -> (u32, u32, Vec<u8>) {
    let [width, depth, slices] = probes.size;
    let mut rgba = Vec::with_capacity(probes.colors.len() * 4);
    for sides in &probes.colors {
        for channel in 0..3 {
            let total: u32 = sides.iter().map(|side| u32::from(side[channel])).sum();
            rgba.push(u8::try_from(total / 6).unwrap_or(u8::MAX));
        }
        rgba.push(u8::MAX);
    }
    (width, depth * slices, rgba)
}

impl Backend {
    /// The final pass's pipeline.
    pub(super) fn make_post_pipeline(
        device: &Device,
        layout: &PipelineLayout,
        pass: &RenderPass,
        samples: u32,
    ) -> Result<GraphicsPipeline, RenderError> {
        // Depth isn't resolved, so with samples it's read as it was drawn.
        let source = if samples > 1 {
            POST_SHADER.replace("texture_depth_2d", "texture_depth_multisampled_2d")
        } else {
            POST_SHADER.to_owned()
        };
        let spirv = vulkane::safe::naga::compile_wgsl(&source)
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

    /// Points `set` at the stencils' pictures.
    pub(super) fn write_stencils(&mut self, set: &DescriptorSet) -> Result<(), RenderError> {
        if self.blank_stencil.is_none() {
            let image = self.upload_image(&[(1, 1, &[0, 0, 0, 0][..])], Format::R8G8B8A8_UNORM)?;
            self.blank_stencil = Some(image);
        }
        for slot in 0..4_u32 {
            let view = self
                .stencil_images
                .get(slot as usize)
                .or(self.blank_stencil.as_ref())
                .map(|image| &image.view);
            if let Some(view) = view {
                set.write_sampled_image(slot + 4, view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            }
        }
        let probes = self.probe_image.as_ref().or(self.blank_stencil.as_ref());
        if let Some(image) = probes {
            set.write_sampled_image(8, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        }
        Ok(())
    }

    /// Uploads `map`'s stencils.
    pub(super) fn load_stencils(&mut self, map: &Map) -> Result<(), RenderError> {
        let mut images = Vec::new();
        for stencil in map.stencils.iter().take(4) {
            let levels: Vec<(u32, u32, &[u8])> = stencil
                .image
                .iter()
                .map(|mip| (mip.width, mip.height, mip.rgba.as_slice()))
                .collect();
            images.push(self.upload_image(&levels, Format::R8G8B8A8_UNORM)?);
        }
        self.stencil_images = images;
        self.probe_image = match &map.probes {
            Some(probes) if !probes.colors.is_empty() => {
                let (width, height, rgba) = probe_slices(probes);
                Some(self.upload_image(&[(width, height, &rgba)], Format::R8G8B8A8_UNORM)?)
            }
            _ => None,
        };
        if let Some(target) = self.scene_target.take() {
            self.write_stencils(&target.set)?;
            self.scene_target = Some(target);
        }
        Ok(())
    }

    /// How big the scene is, or a guess before there is one.
    pub(super) fn scene_size(&self) -> (u32, u32) {
        self.scene_target
            .as_ref()
            .map_or(self.wanted_size, |target| target.size)
    }

    /// Sets how many samples a pixel scenes have, and rebuilds what depends
    /// on it.
    pub(super) fn set_samples(&mut self, samples: u32) -> Result<(), RenderError> {
        // Devices count samples in powers of two, so 3 means 2.
        let most = self.physical.properties().max_samples();
        let samples = 1 << samples.clamp(1, most).ilog2();
        if samples == self.samples {
            return Ok(());
        }
        self.device.wait_idle()?;
        self.samples = samples;
        self.scene_target = None;
        self.scene_pass = make_scene_pass(&self.device, samples)?;
        self.pipelines =
            Self::make_pipelines(&self.device, &self.layouts, &self.scene_pass, samples)?;
        self.post_pipeline =
            Self::make_post_pipeline(&self.device, &self.post_layout, &self.present_pass, samples)?;
        Ok(())
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
        let samples = self.samples;
        let color = Image::new_2d_multisampled(
            &self.device,
            Image2dCreateInfo {
                format: COLOR_FORMAT,
                width: size.0,
                height: size.1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
            },
            samples,
        )?;
        let color = device_image(&self.device, &self.physical, color, false)?;
        let depth = Image::new_2d_multisampled(
            &self.device,
            Image2dCreateInfo {
                format: DEPTH_FORMAT,
                width: size.0,
                height: size.1,
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::SAMPLED,
            },
            samples,
        )?;
        let depth = device_image(&self.device, &self.physical, depth, true)?;
        let resolved = if samples > 1 {
            let image = Image::new_2d(
                &self.device,
                Image2dCreateInfo {
                    format: COLOR_FORMAT,
                    width: size.0,
                    height: size.1,
                    usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
                },
            )?;
            Some(device_image(&self.device, &self.physical, image, false)?)
        } else {
            None
        };
        let mut attachments = vec![&color.view, &depth.view];
        attachments.extend(resolved.as_ref().map(|image| &image.view));
        let framebuffer =
            Framebuffer::new(&self.device, &self.scene_pass, &attachments, size.0, size.1)?;
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
                    descriptor_count: 8,
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
        let shown = resolved.as_ref().unwrap_or(&color);
        set.write_sampled_image(1, &shown.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        set.write_sampled_image(2, &depth.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        set.write_sampler(3, &self.light_sampler);
        let smaa = self
            .smaa
            .target(&self.device, &self.physical, size, shown)?;
        set.write_sampled_image(
            9,
            &smaa.weights().view,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
        self.write_stencils(&set)?;
        self.scene_target = Some(SceneTarget {
            set,
            _pool: pool,
            framebuffer,
            _color: color,
            _depth: depth,
            _resolved: resolved,
            smaa,
            size,
        });
        Ok(())
    }
}
