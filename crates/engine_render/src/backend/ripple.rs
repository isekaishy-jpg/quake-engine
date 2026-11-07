//! Ripples on water near the view: a height field that goes with it, stepped in compute (proposal
//! 0147).

use engine_core::math::Vec3;
use engine_map::{Map, WaterPush};
use engine_shaders::{Module, spirv};
use vulkane::safe::{
    AccessFlags, BufferImageCopy, BufferUsage, ComputePipeline, DescriptorPool, DescriptorPoolSize,
    DescriptorSet, DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorType, Device,
    ImageLayout, PipelineLayout, PipelineStage, PushConstantRange, ShaderModule, ShaderStageFlags,
};

use super::upload::host_buffer;
use super::{Backend, FRAMES, GpuBuffer, GpuImage};
use crate::error::RenderError;
use crate::renderer::RippleHeights;

/// How many cells the field has each way.
const RIPPLE_CELLS: u32 = 512;

/// How wide a cell is, in world units.
const RIPPLE_CELL: f32 = 2.0;

/// How many steps it takes a second.
const RIPPLE_RATE: f32 = 60.0;

/// The most steps it takes in a frame, so that a long frame doesn't make a longer one.
const MOST_STEPS: u32 = 3;

/// The most pushes a frame has.
const MOST_PUSHES: usize = 64;

/// Bytes of a push.
const PUSH_BYTES: usize = 32;

/// Bytes of what a step's told.
const STEP_BYTES: u32 = 16;

/// How many cells make a group of a step.
const GROUP: u32 = 8;

/// What every level's ripples are bound by.
pub(super) struct RippleLayout {
    /// What a step reads and writes.
    set_layout: DescriptorSetLayout,
    /// The layout of its pipeline.
    layout: PipelineLayout,
    /// The step.
    step: ComputePipeline,
}

impl RippleLayout {
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
                binding(2, DescriptorType::STORAGE_BUFFER),
                binding(3, DescriptorType::SAMPLED_IMAGE),
                binding(4, DescriptorType::SAMPLER),
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
        let words = spirv(&Module::Ripple).map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &words)?;
        let step = ComputePipeline::new(device, &layout, &shader, "cs_ripple")?;
        Ok(Self {
            set_layout,
            layout,
            step,
        })
    }
}

/// The ripples of a level.
pub(super) struct GpuRipples {
    /// What's shown, which the final pass reads.
    pub(super) shown: GpuImage,
    /// What a step leaves, before it's kept.
    _next: GpuImage,
    /// What's pushed in, for each frame in flight.
    pushes: Vec<GpuBuffer>,
    /// Steps `shown` into the next, for each frame in flight.
    step_sets: Vec<DescriptorSet>,
    /// Keeps that, for each frame in flight.
    keep_sets: Vec<DescriptorSet>,
    /// Kept alive for the sets.
    _pool: DescriptorPool,
    /// The cell its low corner is at.
    corner: [i32; 2],
    /// How many steps have been taken, by the clock.
    taken: u64,
    /// How many are due this frame.
    due: u32,
    /// How far it's moved for the first of them, in cells.
    moved: [i32; 2],
    /// How many pushes there are.
    count: u32,
    /// What's pushing, until there's a step for it to go in with, as frames come faster than that.
    waiting: Vec<WaterPush>,
}

impl GpuRipples {
    /// Where it is in the world, and how wide, for the final pass.
    #[expect(clippy::cast_precision_loss, reason = "cells are far below 2^24")]
    pub(super) fn place(&self) -> [f32; 3] {
        [
            self.corner[0] as f32 * RIPPLE_CELL,
            self.corner[1] as f32 * RIPPLE_CELL,
            RIPPLE_CELLS as f32 * RIPPLE_CELL,
        ]
    }
}

impl Backend {
    /// Gets `map`'s ripples ready, if it has water.
    pub(super) fn upload_ripples(
        &mut self,
        map: &Map,
        picture_view: &vulkane::safe::ImageView,
    ) -> Result<Option<GpuRipples>, RenderError> {
        if map.pools.is_empty() && map.water_sheets.is_empty() {
            return Ok(None);
        }
        let shown = self.field_image(RIPPLE_CELLS)?;
        let next = self.field_image(RIPPLE_CELLS)?;
        let frames = u32::try_from(FRAMES).unwrap_or(1);
        let size = |descriptor_type, each: u32| DescriptorPoolSize {
            descriptor_type,
            descriptor_count: frames * 2 * each,
        };
        let pool = DescriptorPool::new(
            &self.device,
            frames * 2,
            &[
                size(DescriptorType::SAMPLED_IMAGE, 2),
                size(DescriptorType::STORAGE_IMAGE, 1),
                size(DescriptorType::STORAGE_BUFFER, 1),
                size(DescriptorType::SAMPLER, 1),
            ],
        )?;
        let mut pushes = Vec::with_capacity(FRAMES);
        let (mut step_sets, mut keep_sets) = (Vec::new(), Vec::new());
        for _ in 0..FRAMES {
            let bytes = MOST_PUSHES * PUSH_BYTES;
            let usage = BufferUsage::STORAGE_BUFFER;
            let buffer = host_buffer(&self.device, &self.physical, bytes, usage)?;
            let make = |from: &GpuImage, to: &GpuImage| -> Result<DescriptorSet, RenderError> {
                let set = pool.allocate(&self.ripple_layout.set_layout)?;
                set.write_sampled_image(0, &from.view, ImageLayout::GENERAL);
                set.write_storage_image(1, &to.view, ImageLayout::GENERAL);
                let kind = DescriptorType::STORAGE_BUFFER;
                set.write_buffer(2, kind, &buffer.buffer, 0, bytes as u64);
                set.write_sampled_image(3, picture_view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                set.write_sampler(4, &self.particle_sampler);
                Ok(set)
            };
            step_sets.push(make(&shown, &next)?);
            keep_sets.push(make(&next, &shown)?);
            pushes.push(buffer);
        }
        Ok(Some(GpuRipples {
            shown,
            _next: next,
            pushes,
            step_sets,
            keep_sets,
            _pool: pool,
            corner: [0; 2],
            taken: 0,
            due: 0,
            moved: [0; 2],
            count: 0,
            waiting: Vec::new(),
        }))
    }

    /// Works out this frame's steps, at `time`, round `eye`, with what's pushing in.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a clock's steps and a world's cells are small"
    )]
    pub(super) fn plan_ripples(
        &mut self,
        time: f32,
        eye: Vec3,
        pushes: &[WaterPush],
        slot: usize,
    ) -> Result<(), RenderError> {
        let Some(gpu) = self.map.as_mut().and_then(|map| map.ripples.as_mut()) else {
            return Ok(());
        };
        let owed = (time.max(0.0) * RIPPLE_RATE) as u64;
        // The first frame has no steps owing from before it.
        if gpu.taken == 0 {
            gpu.taken = owed;
        }
        gpu.due = u32::try_from(owed.saturating_sub(gpu.taken))
            .unwrap_or(MOST_STEPS)
            .min(MOST_STEPS);
        gpu.taken = owed;
        let half = (RIPPLE_CELLS / 2).cast_signed();
        let corner = [
            (eye.x / RIPPLE_CELL).floor() as i32 - half,
            (eye.y / RIPPLE_CELL).floor() as i32 - half,
        ];
        gpu.moved = if gpu.due > 0 {
            let moved = [corner[0] - gpu.corner[0], corner[1] - gpu.corner[1]];
            gpu.corner = corner;
            moved
        } else {
            [0; 2]
        };
        let (cx, cy) = (gpu.corner[0] as f32, gpu.corner[1] as f32);
        let room = MOST_PUSHES.saturating_sub(gpu.waiting.len());
        gpu.waiting.extend(pushes.iter().take(room));
        let mut mapped = gpu.pushes[slot].memory.map()?;
        let target = mapped.as_slice_mut();
        let mut count = 0;
        if gpu.due > 0 {
            for push in gpu.waiting.drain(..) {
                let (sine, cosine) = push.turn.sin_cos();
                let numbers = [
                    push.place.x / RIPPLE_CELL - cx,
                    push.place.y / RIPPLE_CELL - cy,
                    push.reach / RIPPLE_CELL,
                    push.strength,
                    cosine,
                    sine,
                    push.picture as f32,
                    0.0,
                ];
                let at = count * PUSH_BYTES;
                let spot = &mut target[at..at + PUSH_BYTES];
                for (bytes, number) in spot.as_chunks_mut::<4>().0.iter_mut().zip(numbers) {
                    *bytes = number.to_le_bytes();
                }
                count += 1;
            }
        }
        gpu.count = u32::try_from(count).unwrap_or(0);
        Ok(())
    }

    /// How high each cell of the ripples is, row by row, and where they are, for what checks them.
    pub(crate) fn read_ripples(&mut self) -> Result<Option<RippleHeights>, RenderError> {
        self.wait_for_frame()?;
        self.device.wait_idle()?;
        let Some(gpu) = self.map.as_ref().and_then(|map| map.ripples.as_ref()) else {
            return Ok(None);
        };
        let cells = RIPPLE_CELLS as usize * RIPPLE_CELLS as usize;
        let mut staging = host_buffer(
            &self.device,
            &self.physical,
            cells * 8,
            BufferUsage::TRANSFER_DST,
        )?;
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
                rec.copy_image_to_buffer(
                    &gpu.shown.image,
                    ImageLayout::GENERAL,
                    &staging.buffer,
                    &[BufferImageCopy::full_2d(RIPPLE_CELLS, RIPPLE_CELLS)],
                );
                Ok(())
            })?;
        let mapped = staging.memory.map()?;
        let bytes = mapped.as_slice();
        let heights = bytes[..cells * 8].as_chunks::<8>().0;
        let heights = heights
            .iter()
            .map(|cell| half(u16::from_le_bytes([cell[0], cell[1]])));
        Ok(Some((heights.collect(), gpu.place())))
    }

    /// Records this frame's steps.
    pub(super) fn record_ripples(&self, rec: &mut vulkane::safe::CommandBufferRecording<'_>) {
        let Some(gpu) = self.map.as_ref().and_then(|map| map.ripples.as_ref()) else {
            return;
        };
        if gpu.due == 0 {
            return;
        }
        let groups = RIPPLE_CELLS / GROUP;
        let layout = &self.ripple_layout;
        rec.bind_compute_pipeline(&layout.step);
        for step in 0..gpu.due {
            let first = step == 0;
            let mut bytes = [0; STEP_BYTES as usize];
            if first {
                bytes[..4].copy_from_slice(&gpu.moved[0].to_le_bytes());
                bytes[4..8].copy_from_slice(&gpu.moved[1].to_le_bytes());
                bytes[8..12].copy_from_slice(&gpu.count.to_le_bytes());
            }
            let passes = [
                (&gpu.step_sets[self.slot], bytes),
                (&gpu.keep_sets[self.slot], keeping()),
            ];
            for (set, push) in passes {
                rec.bind_compute_descriptor_sets(&layout.layout, 0, &[set]);
                rec.push_constants(&layout.layout, ShaderStageFlags::COMPUTE, 0, &push);
                rec.dispatch(groups, groups, 1);
                rec.memory_barrier(
                    PipelineStage::COMPUTE_SHADER,
                    PipelineStage::COMPUTE_SHADER,
                    AccessFlags::SHADER_WRITE,
                    AccessFlags::SHADER_READ | AccessFlags::SHADER_WRITE,
                );
            }
        }
        rec.memory_barrier(
            PipelineStage::COMPUTE_SHADER,
            PipelineStage::FRAGMENT_SHADER,
            AccessFlags::SHADER_WRITE,
            AccessFlags::SHADER_READ,
        );
    }
}

/// A half float as a float.
fn half(bits: u16) -> f32 {
    let sign = if bits >> 15 == 0 { 1.0 } else { -1.0 };
    let power = i32::from(bits >> 10 & 31);
    let part = f32::from(bits & 1023) / 1024.0;
    sign * match power {
        0 => part * 2.0_f32.powi(-14),
        _ => (1.0 + part) * 2.0_f32.powi(power - 15),
    }
}

/// What the step that only keeps is told.
fn keeping() -> [u8; STEP_BYTES as usize] {
    let mut bytes = [0; STEP_BYTES as usize];
    bytes[12] = 1;
    bytes
}
