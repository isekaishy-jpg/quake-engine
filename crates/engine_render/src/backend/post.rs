//! Drawing the scene off-screen, so a final pass can read its colour and depth
//! (proposal 0075).

use vulkane::safe::{
    AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, CullMode, DescriptorPool,
    DescriptorPoolSize, DescriptorSet, DescriptorType, Device, Format, Framebuffer,
    GraphicsPipeline, GraphicsPipelineBuilder, GraphicsShaderStage, Image, Image2dCreateInfo,
    ImageLayout, ImageUsage, PipelineLayout, RenderPass, RenderPassCreateInfo, ShaderModule,
};

use engine_map::{MOST_CLOUDS, Map, ProbeGrid, ShaderCode, SlotImage};
use engine_shaders::{Module, spirv};

use super::smaa::SmaaTarget;
use super::upload::device_image;
use super::water::WaterTarget;
use super::{Backend, DEPTH_FORMAT, FRAME_SIZE, GpuImage, SCENE_FORMAT};
use crate::error::RenderError;

/// Where a scene is drawn before the final pass.
pub(super) struct SceneTarget {
    /// Where water's surfaces are.
    pub(super) water: WaterTarget,
    /// The final pass's set.
    pub(super) set: DescriptorSet,
    /// Kept alive for the set.
    _pool: DescriptorPool,
    /// Where the scene is drawn.
    pub(super) framebuffer: Framebuffer,
    /// Where particles are drawn, over it.
    pub(super) particle_framebuffer: Framebuffer,
    /// The scene's depth, for particles.
    pub(super) particle_set: DescriptorSet,
    /// Kept alive for the set.
    _particle_pool: DescriptorPool,
    /// Whether anything's been drawn here, so that its depth can be read.
    pub(super) drawn: std::cell::Cell<bool>,
    /// Kept alive for the framebuffer.
    _color: GpuImage,
    /// Kept alive for the framebuffer.
    _depth: GpuImage,
    /// Kept alive for the framebuffer, with several samples a pixel.
    _resolved: Option<GpuImage>,
    /// What SMAA draws.
    pub(super) smaa: SmaaTarget,
    /// Where light in the air is drawn.
    pub(super) rays: RayTarget,
    /// How open each pixel is to light from all round.
    pub(super) corners: RayTarget,
    /// Its size.
    pub(super) size: (u32, u32),
}

/// Where light in the air is drawn, or how open pixels are, at half the scene's size each way.
pub(super) struct RayTarget {
    /// Where it's drawn.
    pub(super) framebuffer: Framebuffer,
    /// What's drawn.
    pub(super) image: GpuImage,
    /// Its size.
    pub(super) size: (u32, u32),
}

/// The render pass of light in the air, every pixel of which is drawn.
pub(super) fn make_ray_pass(device: &Device) -> Result<RenderPass, RenderError> {
    Ok(RenderPass::new(
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
    )?)
}

/// What's kept of how open a pixel is: how open, and how far it is from the eye, so that a
/// surface can tell whether it's what was seen there.
const CORNER_FORMAT: Format = Format::R16G16_SFLOAT;

/// The render pass of how open pixels are, every one of which is drawn.
pub(super) fn make_corner_pass(device: &Device) -> Result<RenderPass, RenderError> {
    Ok(RenderPass::new(
        device,
        RenderPassCreateInfo {
            color_attachments: &[AttachmentDescription {
                format: CORNER_FORMAT,
                load_op: AttachmentLoadOp::DONT_CARE,
                store_op: AttachmentStoreOp::STORE,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }],
            depth_attachment: None,
        },
    )?)
}

/// The render pass of a scene, leaving what the final pass reads ready to
/// read. With several samples a pixel, colour resolves to an attachment after
/// the others, and depth stays as it is.
pub(super) fn make_scene_pass(device: &Device, samples: u32) -> Result<RenderPass, RenderError> {
    let resolved = AttachmentDescription {
        format: SCENE_FORMAT,
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
                format: SCENE_FORMAT,
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

/// The scene's pass again, carrying on from what it left, for what's see-through after particles
/// behind it have gone in.
pub(super) fn make_scene_again_pass(
    device: &Device,
    samples: u32,
) -> Result<RenderPass, RenderError> {
    let resolved = AttachmentDescription {
        format: SCENE_FORMAT,
        load_op: AttachmentLoadOp::DONT_CARE,
        store_op: AttachmentStoreOp::STORE,
        initial_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
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
                format: SCENE_FORMAT,
                load_op: AttachmentLoadOp::LOAD,
                store_op: AttachmentStoreOp::STORE,
                initial_layout: color_end,
                final_layout: color_end,
            }],
            depth_attachment: Some(AttachmentDescription {
                format: DEPTH_FORMAT,
                load_op: AttachmentLoadOp::LOAD,
                store_op: AttachmentStoreOp::STORE,
                initial_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }),
        },
        samples,
        resolve,
    )?)
}

/// The pass particles are drawn in: what the scene left, with nothing of its depth attached, as
/// they read that.
pub(super) fn make_particle_pass(device: &Device, samples: u32) -> Result<RenderPass, RenderError> {
    let resolved = AttachmentDescription {
        format: SCENE_FORMAT,
        load_op: AttachmentLoadOp::DONT_CARE,
        store_op: AttachmentStoreOp::STORE,
        initial_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
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
                format: SCENE_FORMAT,
                load_op: AttachmentLoadOp::LOAD,
                store_op: AttachmentStoreOp::STORE,
                initial_layout: color_end,
                final_layout: color_end,
            }],
            depth_attachment: None,
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
    /// A pipeline of the post shader, by the name of its fragment stage.
    pub(super) fn make_post_pipeline(
        device: &Device,
        layout: &PipelineLayout,
        pass: &RenderPass,
        samples: u32,
        entry: &str,
        code: &str,
    ) -> Result<GraphicsPipeline, RenderError> {
        // Depth isn't resolved, so with samples it's read as it was drawn.
        let spirv = spirv(&Module::Post {
            code,
            samples: samples > 1,
        })
        .map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        Ok(GraphicsPipelineBuilder::new(layout, pass)
            .stage(GraphicsShaderStage::Vertex, &shader, "vs_post")
            .stage(GraphicsShaderStage::Fragment, &shader, entry)
            .dynamic_viewport_scissor()
            .cull_mode(CullMode::NONE)
            .depth_test(false, false)
            .build(device)?)
    }

    /// Makes what light in the air, or how open pixels are, is drawn into with `pass`, for a scene
    /// of `size`.
    fn ray_target(
        &self,
        size: (u32, u32),
        pass: &RenderPass,
        format: Format,
    ) -> Result<RayTarget, RenderError> {
        let size = (size.0.div_ceil(2), size.1.div_ceil(2));
        let image = Image::new_2d(
            &self.device,
            Image2dCreateInfo {
                format,
                width: size.0,
                height: size.1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
            },
        )?;
        let image = device_image(&self.device, &self.physical, image, false)?;
        let framebuffer = Framebuffer::new(&self.device, pass, &[&image.view], size.0, size.1)?;
        Ok(RayTarget {
            framebuffer,
            image,
            size,
        })
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
    /// Puts a sky's pictures on the GPU, for the sky and for the final pass.
    pub(super) fn upload_sky_pictures(&mut self, map: &Map) -> Result<(), RenderError> {
        let mut upload = |image: &SlotImage| {
            let levels: Vec<(u32, u32, &[u8])> = image
                .iter()
                .map(|mip| (mip.width, mip.height, mip.rgba.as_slice()))
                .collect();
            self.upload_image(&levels, Format::R8G8B8A8_UNORM)
        };
        let sky = map.sky.as_ref();
        let clouds = sky.iter().flat_map(|sky| &sky.clouds);
        let images = clouds.filter_map(|clouds| clouds.image.as_ref());
        let mut pictures = Vec::new();
        for image in images.take(MOST_CLOUDS) {
            pictures.push(upload(image)?);
        }
        let rays = sky.and_then(|sky| sky.rays.as_ref());
        let ray_picture = rays.and_then(|rays| rays.picture.as_ref());
        let ray_picture = ray_picture.map(&mut upload).transpose()?;
        self.cloud_pictures = pictures;
        self.sky_ray_picture = ray_picture;
        Ok(())
    }

    /// Tells the final pass of a sky's pictures.
    pub(super) fn write_sky_pictures(&self, set: &DescriptorSet) {
        let Some(blank) = &self.sky_blank else {
            return;
        };
        for binding in 0..MOST_CLOUDS {
            let image = self.cloud_pictures.get(binding).unwrap_or(blank);
            #[expect(clippy::cast_possible_truncation, reason = "four of them")]
            let at = 15 + binding as u32;
            set.write_sampled_image(at, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        }
        let rays = self.sky_ray_picture.as_ref().unwrap_or(blank);
        set.write_sampled_image(19, &rays.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
    }

    pub(super) fn load_stencils(&mut self, map: &Map) -> Result<(), RenderError> {
        let mut images = Vec::new();
        for stencil in map.stencils.iter().take(4) {
            let levels: Vec<(u32, u32, &[u8])> = stencil
                .image
                .iter()
                .map(|mip| (mip.width, mip.height, mip.rgba.as_slice()))
                .collect();
            images.push(self.upload_image(&levels, Format::R8G8B8A8_SRGB)?);
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
            self.write_sky_pictures(&target.set);
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
        self.particle_pass = make_particle_pass(&self.device, samples)?;
        self.scene_again_pass = make_scene_again_pass(&self.device, samples)?;
        self.particle_pipelines = Self::make_particle_pipelines(
            &self.device,
            &self.layouts,
            &self.particle_pass,
            samples,
            "",
        )?;
        // A map's textures know shaders by their place, so that has to stay.
        let surfaces = self.pipelines.surfaces.iter().skip(1);
        let codes: Vec<String> = surfaces.map(|surface| surface.code.clone()).collect();
        self.pipelines =
            Self::make_pipelines(&self.device, &self.layouts, &self.scene_pass, samples)?;
        self.post_pipeline = Self::make_post_pipeline(
            &self.device,
            &self.post_layout,
            &self.present_pass,
            samples,
            "fs_post",
            &self.post_code,
        )?;
        self.ray_pipeline = Self::make_post_pipeline(
            &self.device,
            &self.post_layout,
            &self.ray_pass,
            samples,
            "fs_rays",
            "",
        )?;
        self.corner_pipeline = Self::make_post_pipeline(
            &self.device,
            &self.post_layout,
            &self.corner_pass,
            samples,
            "fs_corners",
            "",
        )?;
        for code in codes {
            let surface = Self::make_surface_pipelines(
                &self.device,
                &self.layouts,
                &self.scene_pass,
                samples,
                &code,
            )?;
            self.pipelines.surfaces.push(surface);
        }
        self.remake_map_particles()
    }

    /// Draws the final pass with `shader`, or with none. If it won't compile
    /// that's noted, and there's none.
    pub(super) fn use_post_shader(
        &mut self,
        shader: Option<&ShaderCode>,
    ) -> Result<(), RenderError> {
        let code = shader.map_or("", |shader| shader.code.as_str());
        if code == self.post_code {
            return Ok(());
        }
        let made = Self::make_post_pipeline(
            &self.device,
            &self.post_layout,
            &self.present_pass,
            self.samples,
            "fs_post",
            code,
        );
        match made {
            Ok(pipeline) => {
                self.post_pipeline = pipeline;
                code.clone_into(&mut self.post_code);
            }
            Err(error) => {
                let name = shader.map_or("", |shader| shader.name.as_str());
                self.shader_problems.push(format!("{name}: {error}"));
                return self.use_post_shader(None);
            }
        }
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
                format: SCENE_FORMAT,
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
                    format: SCENE_FORMAT,
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
                    descriptor_count: 12,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLER,
                    descriptor_count: 2,
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
        let mut drawn_over = vec![&color.view];
        drawn_over.extend(resolved.as_ref().map(|image| &image.view));
        let particle_framebuffer = Framebuffer::new(
            &self.device,
            &self.particle_pass,
            &drawn_over,
            size.0,
            size.1,
        )?;
        let particle_pool = DescriptorPool::new(
            &self.device,
            1,
            &[DescriptorPoolSize {
                descriptor_type: DescriptorType::SAMPLED_IMAGE,
                descriptor_count: 1,
            }],
        )?;
        let particle_set = particle_pool.allocate(&self.particle_depth_layout)?;
        particle_set.write_sampled_image(2, &depth.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
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
        let rays = self.ray_target(size, &self.ray_pass, SCENE_FORMAT)?;
        set.write_sampled_image(10, &rays.image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        let corners = self.ray_target(size, &self.corner_pass, CORNER_FORMAT)?;
        // Surfaces read it, and the map's set was made before this was.
        if let Some(map) = &self.map {
            map.frame_set.write_sampled_image(
                9,
                &corners.image.view,
                ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );
        }
        let water = self.water_target(size)?;
        set.write_sampled_image(11, &water.image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        if self.water_pictures.is_none() {
            self.water_pictures = Some(self.upload_array(&[], Format::R8G8B8A8_SRGB)?);
        }
        self.write_water_pictures(&set);
        set.write_sampler(13, &self.particle_sampler);
        if self.ripple_blank.is_none() {
            self.ripple_blank = Some(self.field_image(1)?);
        }
        self.write_ripples(&set);
        if self.sky_blank.is_none() {
            let white = [255_u8; 4];
            self.sky_blank = Some(self.upload_image(&[(1, 1, &white)], Format::R8G8B8A8_UNORM)?);
        }
        self.write_sky_pictures(&set);
        self.write_stencils(&set)?;
        self.scene_target = Some(SceneTarget {
            particle_framebuffer,
            particle_set,
            _particle_pool: particle_pool,
            drawn: std::cell::Cell::new(false),
            set,
            _pool: pool,
            framebuffer,
            _color: color,
            _depth: depth,
            _resolved: resolved,
            smaa,
            rays,
            corners,
            water,
            size,
        });
        Ok(())
    }
}
