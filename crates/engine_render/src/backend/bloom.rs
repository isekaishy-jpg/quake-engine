//! Bloom (proposal 0150): what's brighter than a screen can show, blurred wide by going down a
//! chain of images each half the last and back up again. The final pass adds it.

use std::cell::Cell;

use engine_shaders::{Module, spirv};
use vulkane::safe::{
    AccessFlags, AttachmentDescription, AttachmentLoadOp, AttachmentStoreOp, ClearValue, CullMode,
    DescriptorPool, DescriptorPoolSize, DescriptorSet, DescriptorSetLayout,
    DescriptorSetLayoutBinding, DescriptorType, Device, Format, Framebuffer, GraphicsPipeline,
    GraphicsPipelineBuilder, GraphicsShaderStage, Image, Image2dCreateInfo, ImageLayout,
    ImageUsage, PhysicalDevice, PipelineLayout, PipelineStage, PushConstantRange, RenderPass,
    RenderPassCreateInfo, Sampler, SamplerCreateInfo, ShaderModule, ShaderStageFlags,
};

use super::GpuImage;
use super::upload::device_image;
use crate::error::RenderError;

/// What each image of the chain holds: light, which can be brighter than a screen shows.
const FORMAT: Format = Format::R16G16B16A16_SFLOAT;

/// The most images the chain goes down through.
pub(super) const MOST_LEVELS: usize = 6;

/// The smallest an image of the chain gets, either way.
const SMALLEST: u32 = 8;

/// The bytes of what a step's told: threshold, knee, exposure and spread.
const STEP_BYTES: u32 = 16;

/// What each step's told.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct BloomStep {
    /// How bright light has to be, once exposed, before it glows.
    pub(super) threshold: f32,
    /// How softly it comes in below that.
    pub(super) knee: f32,
    /// What exposes the scene.
    pub(super) exposure: f32,
}

impl BloomStep {
    /// As the shader has it.
    fn bytes(self) -> [u8; STEP_BYTES as usize] {
        let mut bytes = [0; STEP_BYTES as usize];
        let values = [self.threshold, self.knee, self.exposure, 1.0];
        for (chunk, value) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(values) {
            *chunk = value.to_le_bytes();
        }
        bytes
    }
}

/// One image of the chain.
struct Level {
    /// Where it's drawn.
    framebuffer: Framebuffer,
    /// What's drawn.
    image: GpuImage,
    /// Its size.
    size: (u32, u32),
    /// What it reads on the way down: the one above it, or the scene.
    down: DescriptorSet,
    /// What it reads on the way up: the one below it, if there is one.
    up: Option<DescriptorSet>,
}

/// The chain for a scene of one size.
pub(super) struct BloomTarget {
    /// Its images, largest first.
    levels: Vec<Level>,
    /// Kept alive for the sets.
    _pool: DescriptorPool,
    /// Whether the first image has been started off, for the final pass to read.
    ready: Cell<bool>,
}

impl BloomTarget {
    /// The first image, which holds all of it once it's been back up.
    pub(super) fn image(&self) -> &GpuImage {
        &self.levels[0].image
    }

    /// How many images there are, each of which adds its share.
    pub(super) fn count(&self) -> usize {
        self.levels.len()
    }
}

/// What bloom's passes need, whatever the size.
pub(super) struct Bloom {
    /// The way down, starting each image afresh.
    down_pass: RenderPass,
    /// The way up, adding to what's there.
    up_pass: RenderPass,
    /// The layout of what a step reads.
    set_layout: DescriptorSetLayout,
    /// The layout of its draws.
    layout: PipelineLayout,
    /// How it reads.
    sampler: Sampler,
    /// The first step down, from the scene.
    down_first: GraphicsPipeline,
    /// A step down.
    down: GraphicsPipeline,
    /// A step up.
    up: GraphicsPipeline,
}

/// A pass drawing one image of the chain, from `load` and as `initial`.
fn chain_pass(
    device: &Device,
    load: AttachmentLoadOp,
    initial: ImageLayout,
) -> Result<RenderPass, RenderError> {
    Ok(RenderPass::new(
        device,
        RenderPassCreateInfo {
            color_attachments: &[AttachmentDescription {
                format: FORMAT,
                load_op: load,
                store_op: AttachmentStoreOp::STORE,
                initial_layout: initial,
                final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            }],
            depth_attachment: None,
        },
    )?)
}

impl Bloom {
    /// Makes the passes.
    pub(super) fn new(device: &Device) -> Result<Self, RenderError> {
        let down_pass = chain_pass(device, AttachmentLoadOp::CLEAR, ImageLayout::UNDEFINED)?;
        let up_pass = chain_pass(
            device,
            AttachmentLoadOp::LOAD,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
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
                binding(0, DescriptorType::SAMPLED_IMAGE),
                binding(1, DescriptorType::SAMPLER),
            ],
        )?;
        let layout = PipelineLayout::with_push_constants(
            device,
            &[&set_layout],
            &[PushConstantRange {
                stage_flags: ShaderStageFlags::FRAGMENT,
                offset: 0,
                size: STEP_BYTES,
            }],
        )?;
        let sampler = Sampler::new(device, SamplerCreateInfo::default())?;
        let spirv = spirv(&Module::Bloom).map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &spirv)?;
        let pipeline = |pass: &RenderPass, entry, adds| {
            GraphicsPipelineBuilder::new(&layout, pass)
                .stage(GraphicsShaderStage::Vertex, &shader, "vs_full")
                .stage(GraphicsShaderStage::Fragment, &shader, entry)
                .dynamic_viewport_scissor()
                .cull_mode(CullMode::NONE)
                .depth_test(false, false)
                .additive_blending(adds)
                .build(device)
        };
        let down_first = pipeline(&down_pass, "fs_down_first", false)?;
        let down = pipeline(&down_pass, "fs_down", false)?;
        let up = pipeline(&up_pass, "fs_up", true)?;
        Ok(Self {
            down_pass,
            up_pass,
            set_layout,
            layout,
            sampler,
            down_first,
            down,
            up,
        })
    }

    /// A set reading `image`.
    fn reading(
        &self,
        pool: &DescriptorPool,
        image: &GpuImage,
    ) -> Result<DescriptorSet, RenderError> {
        let set = pool.allocate(&self.set_layout)?;
        set.write_sampled_image(0, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        set.write_sampler(1, &self.sampler);
        Ok(set)
    }

    /// Makes the chain for a scene of `size` whose colour is `scene`.
    pub(super) fn target(
        &self,
        device: &Device,
        physical: &PhysicalDevice,
        size: (u32, u32),
        scene: &GpuImage,
    ) -> Result<BloomTarget, RenderError> {
        let mut sizes = Vec::new();
        let mut next = (size.0.div_ceil(2).max(1), size.1.div_ceil(2).max(1));
        loop {
            sizes.push(next);
            next = (next.0.div_ceil(2), next.1.div_ceil(2));
            if sizes.len() == MOST_LEVELS || next.0 < SMALLEST || next.1 < SMALLEST {
                break;
            }
        }
        #[expect(clippy::cast_possible_truncation, reason = "a few levels")]
        let sets = (sizes.len() * 2) as u32;
        let pool = DescriptorPool::new(
            device,
            sets,
            &[
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
        let mut images = Vec::with_capacity(sizes.len());
        for &(width, height) in &sizes {
            let image = Image::new_2d(
                device,
                Image2dCreateInfo {
                    format: FORMAT,
                    width,
                    height,
                    usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
                },
            )?;
            images.push(device_image(device, physical, image, false)?);
        }
        let mut levels = Vec::with_capacity(sizes.len());
        for (at, &size) in sizes.iter().enumerate() {
            let above = if at == 0 { scene } else { &images[at - 1] };
            let down = self.reading(&pool, above)?;
            let up = images
                .get(at + 1)
                .map(|below| self.reading(&pool, below))
                .transpose()?;
            let framebuffer =
                Framebuffer::new(device, &self.down_pass, &[&images[at].view], size.0, size.1)?;
            levels.push((framebuffer, size, down, up));
        }
        let levels = levels
            .into_iter()
            .zip(images)
            .map(|((framebuffer, size, down, up), image)| Level {
                framebuffer,
                image,
                size,
                down,
                up,
            })
            .collect();
        Ok(BloomTarget {
            levels,
            _pool: pool,
            ready: Cell::new(false),
        })
    }

    /// Draws `level` with `pipeline` reading `set`, in `pass`.
    fn record_level(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        (pass, pipeline): (&RenderPass, Option<&GraphicsPipeline>),
        level: &Level,
        set: &DescriptorSet,
        step: BloomStep,
    ) {
        let empty = [ClearValue::Color([0.0; 4])];
        rec.begin_render_pass_ext(pass, &level.framebuffer, &empty);
        if let Some(pipeline) = pipeline {
            #[expect(clippy::cast_precision_loss, reason = "image sizes are far below 2^24")]
            rec.set_viewport(0.0, 0.0, level.size.0 as f32, level.size.1 as f32);
            rec.set_scissor(0, 0, level.size.0, level.size.1);
            rec.bind_graphics_pipeline(pipeline);
            rec.bind_graphics_descriptor_sets(&self.layout, 0, &[set]);
            rec.push_constants(&self.layout, ShaderStageFlags::FRAGMENT, 0, &step.bytes());
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

    /// Records the chain down and back up if `wanted`. Otherwise it only makes sure the final pass
    /// has something it can read.
    pub(super) fn record(
        &self,
        rec: &mut vulkane::safe::CommandBufferRecording<'_>,
        target: &BloomTarget,
        step: BloomStep,
        wanted: bool,
    ) {
        let started = target.ready.replace(true);
        if !wanted {
            if !started {
                let first = &target.levels[0];
                self.record_level(rec, (&self.down_pass, None), first, &first.down, step);
            }
            return;
        }
        for (at, level) in target.levels.iter().enumerate() {
            let pipeline = if at == 0 {
                &self.down_first
            } else {
                &self.down
            };
            self.record_level(
                rec,
                (&self.down_pass, Some(pipeline)),
                level,
                &level.down,
                step,
            );
        }
        for level in target.levels.iter().rev() {
            if let Some(up) = &level.up {
                self.record_level(rec, (&self.up_pass, Some(&self.up)), level, up, step);
            }
        }
    }
}
