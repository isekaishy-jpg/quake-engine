//! Fields on the GPU: pictures worked out a step at a time, by a level's own compute shaders
//! (proposal 0145).

use std::sync::Arc;

use engine_map::{FieldDef, Map, ShaderCode, SlotImage};
use engine_shaders::{Module, spirv};
use vulkane::safe::{
    AccessFlags, BufferImageCopy, BufferUsage, ComputePipeline, DescriptorPool, DescriptorPoolSize,
    DescriptorSet, DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorType, Device, Format,
    Image, Image2dCreateInfo, ImageBarrier, ImageLayout, ImageUsage, PipelineLayout, PipelineStage,
    PushConstantRange, ShaderModule, ShaderStageFlags,
};

use super::upload::{device_image, host_buffer};
use super::{Backend, GpuImage, ParticlePictures, ParticlePipelines};
use crate::error::RenderError;

/// The format of a field's images.
const FIELD_FORMAT: Format = Format::R16G16B16A16_SFLOAT;

/// Bytes in what a step's told.
const STEP_BYTES: u32 = 48;

/// How many cells a group of a step covers each way.
const GROUP: u32 = 8;

/// The most steps a field takes in a frame, so that a long frame doesn't make a longer one.
const MOST_STEPS: u64 = 4;

/// What every field's steps are bound by.
pub(super) struct FieldLayout {
    /// What a step reads and writes.
    set_layout: DescriptorSetLayout,
    /// The layout of its pipelines.
    layout: PipelineLayout,
}

impl FieldLayout {
    /// Makes them.
    pub(super) fn new(device: &Device) -> Result<Self, RenderError> {
        let binding = |binding, descriptor_type| DescriptorSetLayoutBinding {
            binding,
            descriptor_type,
            descriptor_count: 1,
            stage_flags: ShaderStageFlags::COMPUTE,
        };
        let set_layout = DescriptorSetLayout::new(
            device,
            &[
                binding(0, DescriptorType::SAMPLED_IMAGE),
                binding(1, DescriptorType::STORAGE_IMAGE),
            ],
        )?;
        let layout = PipelineLayout::with_push_constants(
            device,
            &[&set_layout],
            &[PushConstantRange {
                stage_flags: ShaderStageFlags::COMPUTE,
                offset: 0,
                size: STEP_BYTES,
            }],
        )?;
        Ok(Self { set_layout, layout })
    }
}

/// A field on the GPU.
pub(super) struct GpuField {
    /// What it's called, which is how a material asks for it.
    pub(super) name: String,
    /// How wide it is, each way.
    size: u32,
    /// How many steps a second.
    rate: f32,
    /// What it was given.
    params: [f32; 8],
    /// What's read: by a step, and by what shows it.
    pub(crate) shown: GpuImage,
    /// What a step writes.
    _next: GpuImage,
    /// Reads `shown` and writes `_next`.
    step_set: DescriptorSet,
    /// Reads `_next` and writes `shown`.
    keep_set: DescriptorSet,
    /// Kept alive for the sets.
    _pool: DescriptorPool,
    /// Works out a step.
    step: ComputePipeline,
    /// Keeps what it worked out.
    keep: ComputePipeline,
    /// How many steps it's had since the level began.
    taken: u64,
    /// How many are due this frame.
    due: u64,
}

impl Backend {
    /// One of a field's images, cleared, ready to be read and written.
    fn field_image(&self, size: u32) -> Result<GpuImage, RenderError> {
        let image = Image::new_2d(
            &self.device,
            Image2dCreateInfo {
                format: FIELD_FORMAT,
                width: size,
                height: size,
                usage: ImageUsage::SAMPLED | ImageUsage::STORAGE | ImageUsage::TRANSFER_DST,
            },
        )?;
        let image = device_image(&self.device, &self.physical, image, false)?;
        let bytes = (size as usize) * (size as usize) * 8;
        let mut zeros = host_buffer(
            &self.device,
            &self.physical,
            bytes,
            BufferUsage::TRANSFER_SRC,
        )?;
        zeros.memory.map()?.as_slice_mut()[..bytes].fill(0);
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
                rec.image_barrier(
                    PipelineStage::TOP_OF_PIPE,
                    PipelineStage::TRANSFER,
                    ImageBarrier::color(
                        &image.image,
                        ImageLayout::UNDEFINED,
                        ImageLayout::TRANSFER_DST_OPTIMAL,
                        AccessFlags::NONE,
                        AccessFlags::TRANSFER_WRITE,
                    ),
                );
                rec.copy_buffer_to_image(
                    &zeros.buffer,
                    &image.image,
                    ImageLayout::TRANSFER_DST_OPTIMAL,
                    &[BufferImageCopy::full_2d(size, size)],
                );
                rec.image_barrier(
                    PipelineStage::TRANSFER,
                    PipelineStage::COMPUTE_SHADER,
                    ImageBarrier::color(
                        &image.image,
                        ImageLayout::TRANSFER_DST_OPTIMAL,
                        ImageLayout::GENERAL,
                        AccessFlags::TRANSFER_WRITE,
                        AccessFlags::SHADER_READ,
                    ),
                );
                Ok(())
            })?;
        Ok(image)
    }

    /// Puts one field on the GPU. One whose shader's wrong doesn't step, and says so.
    fn upload_field(&mut self, field: &FieldDef) -> Result<Option<GpuField>, RenderError> {
        let code = &field.shader.code;
        let module = Module::Field(code);
        let words = if code.trim().is_empty() {
            Err("there's no such shader".to_owned())
        } else {
            spirv(&module)
        };
        let words = match words {
            Ok(words) => words,
            Err(problem) => {
                self.shader_problems
                    .push(format!("field {}: {problem}", field.name));
                return Ok(None);
            }
        };
        let shader = ShaderModule::from_spirv(&self.device, &words)?;
        let layout = &self.field_layout.layout;
        let step = ComputePipeline::new(&self.device, layout, &shader, "cs_field")?;
        let keep = ComputePipeline::new(&self.device, layout, &shader, "cs_field_keep")?;
        let shown = self.field_image(field.size)?;
        let next = self.field_image(field.size)?;
        let pool = DescriptorPool::new(
            &self.device,
            2,
            &[
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: 2,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::STORAGE_IMAGE,
                    descriptor_count: 2,
                },
            ],
        )?;
        let step_set = pool.allocate(&self.field_layout.set_layout)?;
        step_set.write_sampled_image(0, &shown.view, ImageLayout::GENERAL);
        step_set.write_storage_image(1, &next.view, ImageLayout::GENERAL);
        let keep_set = pool.allocate(&self.field_layout.set_layout)?;
        keep_set.write_sampled_image(0, &next.view, ImageLayout::GENERAL);
        keep_set.write_storage_image(1, &shown.view, ImageLayout::GENERAL);
        Ok(Some(GpuField {
            name: field.name.clone(),
            size: field.size,
            rate: field.rate,
            params: field.params,
            shown,
            _next: next,
            step_set,
            keep_set,
            _pool: pool,
            step,
            keep,
            taken: 0,
            due: 0,
        }))
    }

    /// Puts what `map`'s effects show on the GPU.
    pub(super) fn upload_particle_pictures(
        &self,
        map: &Map,
    ) -> Result<ParticlePictures, RenderError> {
        let pictures: Vec<&SlotImage> = map.effect_pictures.iter().collect();
        let image = self.upload_array(&pictures, Format::R8G8B8A8_SRGB)?;
        let pool = DescriptorPool::new(
            &self.device,
            1,
            &[
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: 1,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLER,
                    descriptor_count: 1,
                },
            ],
        )?;
        let set = pool.allocate(&self.particle_picture_layout)?;
        set.write_sampled_image(0, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        set.write_sampler(1, &self.particle_sampler);
        Ok(ParticlePictures {
            _image: image,
            set,
            _pool: pool,
        })
    }

    /// The pipelines of each shader `map`'s effects have of their own. One that's wrong draws as
    /// if it wasn't there, and says so.
    pub(super) fn upload_particle_shaders(
        &mut self,
        shaders: &[Arc<ShaderCode>],
    ) -> Result<Vec<ParticlePipelines>, RenderError> {
        let mut out = Vec::with_capacity(shaders.len());
        for shader in shaders {
            let made = Self::make_particle_pipelines(
                &self.device,
                &self.layouts,
                &self.particle_pass,
                self.samples,
                &shader.code,
            );
            out.push(match made {
                Ok(pipelines) => pipelines,
                Err(problem) => {
                    self.shader_problems
                        .push(format!("{}: {}", shader.name, problem.0));
                    Self::make_particle_pipelines(
                        &self.device,
                        &self.layouts,
                        &self.particle_pass,
                        self.samples,
                        "",
                    )?
                }
            });
        }
        Ok(out)
    }

    /// Puts `map`'s fields on the GPU.
    pub(super) fn upload_fields(&mut self, map: &Map) -> Result<Vec<GpuField>, RenderError> {
        let mut fields = Vec::with_capacity(map.fields.len());
        for field in &map.fields {
            fields.extend(self.upload_field(field)?);
        }
        Ok(fields)
    }

    /// Works out how many steps each field has due at `time`.
    pub(super) fn plan_fields(&mut self, time: f32) {
        self.field_time = time;
        let Some(map) = &mut self.map else {
            return;
        };
        for field in &mut map.fields {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "steps since the level began"
            )]
            let now = (f64::from(time) * f64::from(field.rate)).max(0.0) as u64;
            field.due = now.saturating_sub(field.taken).min(MOST_STEPS);
            field.taken = now;
        }
    }

    /// Records the steps that are due. What draws reads what they leave.
    pub(super) fn record_fields(&self, rec: &mut vulkane::safe::CommandBufferRecording<'_>) {
        let Some(map) = &self.map else {
            return;
        };
        let mut any = false;
        for field in &map.fields {
            let mut words = [0_u32; 12];
            words[0] = self.field_time.to_bits();
            words[1] = field.size;
            words[2] = field.size;
            for (word, param) in words[4..].iter_mut().zip(field.params) {
                *word = param.to_bits();
            }
            let mut bytes = [0; STEP_BYTES as usize];
            for (chunk, word) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(words) {
                *chunk = word.to_le_bytes();
            }
            let groups = field.size.div_ceil(GROUP);
            for _ in 0..field.due {
                any = true;
                let passes = [
                    (&field.step, &field.step_set),
                    (&field.keep, &field.keep_set),
                ];
                for (pipeline, set) in passes {
                    rec.bind_compute_pipeline(pipeline);
                    rec.bind_compute_descriptor_sets(&self.field_layout.layout, 0, &[set]);
                    rec.push_constants(
                        &self.field_layout.layout,
                        ShaderStageFlags::COMPUTE,
                        0,
                        &bytes,
                    );
                    rec.dispatch(groups, groups, 1);
                    rec.memory_barrier(
                        PipelineStage::COMPUTE_SHADER,
                        PipelineStage::COMPUTE_SHADER,
                        AccessFlags::SHADER_WRITE,
                        AccessFlags::SHADER_READ | AccessFlags::SHADER_WRITE,
                    );
                }
            }
        }
        if any {
            rec.memory_barrier(
                PipelineStage::COMPUTE_SHADER,
                PipelineStage::FRAGMENT_SHADER,
                AccessFlags::SHADER_WRITE,
                AccessFlags::SHADER_READ,
            );
        }
    }
}
