//! Compute particles on the GPU: a ring of slots for each emitter, stepped each frame (proposal
//! 0146).

use std::sync::Arc;

use engine_map::{Map, ShaderCode, SurfacePoint};
use engine_shaders::{Module, spirv};
use vulkane::safe::{
    AccessFlags, BufferCopy, BufferUsage, ComputePipeline, DescriptorPool, DescriptorPoolSize,
    DescriptorSet, DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorType, Device,
    ImageLayout, PhysicalDevice, PipelineLayout, PipelineStage, PushConstantRange, ShaderModule,
    ShaderStageFlags,
};

use super::field::GpuField;
use super::upload::host_buffer;
use super::{Backend, FRAMES, GpuBuffer, ParticlePictures, ParticlePipelines};
use crate::error::RenderError;
use crate::sprite::{ParticleSim, SIM_EMITTER_BYTES, sim_emitter_bytes};

/// The most compute particles there can be at once.
const MOST_SLOTS: u32 = 1 << 21;

/// Bytes in one.
const SLOT_BYTES: u64 = 32;

/// The most emitters running at once.
pub(crate) const MOST_SIMS: usize = 1024;

/// How many a group of a step covers.
const GROUP: u32 = 64;

/// Bytes pushed for a step or a draw.
pub(super) const SIM_PUSH: u32 = 16;

/// Where one emitter's slots are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SimRange {
    /// Which emitter of which effect it is.
    id: u64,
    /// Its first slot.
    first: u32,
    /// How many it has.
    count: u32,
    /// The next to use.
    head: u32,
}

/// What's stepped this frame.
#[derive(Clone, Copy, Debug)]
pub(super) struct SimRun {
    /// Which entry of the frame's emitters it is.
    pub(super) emitter: u32,
    /// How many slots it has.
    pub(super) count: u32,
    /// Which of the level's particle shaders draws it, plus one, or 0 for none.
    pub(super) shader: u32,
    /// Which of the level's motions it has, plus one, or 0 for none.
    motion: u32,
    /// Which of the level's fields it reads, plus one, or 0 for none.
    field: u32,
}

/// What every step's bound by.
pub(super) struct SimLayout {
    /// What's read and written.
    pub(super) set_layout: DescriptorSetLayout,
    /// The field it reads.
    pub(super) field_layout: DescriptorSetLayout,
    /// The layout of a step.
    layout: PipelineLayout,
}

impl SimLayout {
    /// Makes them, with `depth` as how the scene's depth is read.
    pub(super) fn new(device: &Device, depth: &DescriptorSetLayout) -> Result<Self, RenderError> {
        let stages = ShaderStageFlags::COMPUTE | ShaderStageFlags::VERTEX;
        let buffer = |binding| DescriptorSetLayoutBinding {
            binding,
            descriptor_type: DescriptorType::STORAGE_BUFFER,
            descriptor_count: 1,
            stage_flags: stages,
        };
        let set_layout = DescriptorSetLayout::new(device, &[buffer(0), buffer(1), buffer(2)])?;
        let field_layout = DescriptorSetLayout::new(
            device,
            &[DescriptorSetLayoutBinding {
                binding: 0,
                descriptor_type: DescriptorType::SAMPLED_IMAGE,
                descriptor_count: 1,
                stage_flags: ShaderStageFlags::COMPUTE,
            }],
        )?;
        let layout = PipelineLayout::with_push_constants(
            device,
            &[&set_layout, depth, &field_layout],
            &[PushConstantRange {
                stage_flags: ShaderStageFlags::COMPUTE,
                offset: 0,
                size: SIM_PUSH,
            }],
        )?;
        Ok(Self {
            set_layout,
            field_layout,
            layout,
        })
    }
}

/// The compute particles of a level.
pub(super) struct GpuSims {
    /// Every slot.
    slots: GpuBuffer,
    /// What each frame in flight says of its emitters.
    emitters: Vec<GpuBuffer>,
    /// Places on meshes, for what starts on them.
    _surfaces: GpuBuffer,
    /// What steps and draws read, for each frame in flight.
    pub(super) sets: Vec<DescriptorSet>,
    /// Kept alive for the sets.
    _pool: DescriptorPool,
    /// A set with each field in it, and one for none.
    field_sets: Vec<DescriptorSet>,
    /// Kept alive for those.
    _field_pool: DescriptorPool,
    /// A step with nothing of its own.
    plain: ComputePipeline,
    /// A step for each of the level's motions.
    motions: Vec<ComputePipeline>,
    /// Those motions, for making them again.
    codes: Vec<Arc<ShaderCode>>,
    /// The view that drew the depth the next step reads, and the way back from it.
    seen: Option<([f32; 16], [f32; 16])>,
    /// Where each emitter's slots are, lowest first.
    ranges: Vec<SimRange>,
    /// What's stepped, for each frame in flight.
    pub(super) runs: [Vec<SimRun>; FRAMES],
}

/// Bytes of a place on a mesh.
const SURFACE_BYTES: usize = 32;

/// `places` for compute to read, with room for one so that there's always a buffer.
fn surface_buffer(
    device: &Device,
    physical: &PhysicalDevice,
    places: &[SurfacePoint],
) -> Result<GpuBuffer, RenderError> {
    let size = places.len().max(1) * SURFACE_BYTES;
    let mut buffer = host_buffer(device, physical, size, BufferUsage::STORAGE_BUFFER)?;
    let mut mapped = buffer.memory.map()?;
    let target = mapped.as_slice_mut();
    for (spot, point) in target
        .as_chunks_mut::<SURFACE_BYTES>()
        .0
        .iter_mut()
        .zip(places)
    {
        let (place, facing) = (point.place, point.facing);
        let numbers = [
            place.x, place.y, place.z, 0.0, facing.x, facing.y, facing.z, 0.0,
        ];
        for (bytes, number) in spot.as_chunks_mut::<4>().0.iter_mut().zip(numbers) {
            *bytes = number.to_le_bytes();
        }
    }
    drop(mapped);
    Ok(buffer)
}

/// The compute module of `code`, as a pipeline.
fn step_pipeline(
    device: &Device,
    layout: &SimLayout,
    code: &str,
    samples: bool,
) -> Result<ComputePipeline, RenderError> {
    let module = Module::Motion { code, samples };
    let words = spirv(&module).map_err(RenderError)?;
    let shader = ShaderModule::from_spirv(device, &words)?;
    Ok(ComputePipeline::new(
        device,
        &layout.layout,
        &shader,
        "cs_motion",
    )?)
}

impl Backend {
    /// Gets `map`'s compute particles ready, if any of its effects has them.
    pub(super) fn upload_sims(
        &mut self,
        map: &Map,
        fields: &[GpuField],
    ) -> Result<Option<GpuSims>, RenderError> {
        let wanted = map
            .effects
            .iter()
            .any(|effect| effect.emitters.iter().any(|emitter| emitter.sim));
        if !wanted {
            return Ok(None);
        }
        let usage = BufferUsage::STORAGE_BUFFER | BufferUsage::TRANSFER_SRC;
        let bytes = usize::try_from(u64::from(MOST_SLOTS) * SLOT_BYTES).unwrap_or(usize::MAX);
        let slots = self.scratch_buffer(bytes, usage)?;
        let mut emitters = Vec::with_capacity(FRAMES);
        for _ in 0..FRAMES {
            emitters.push(host_buffer(
                &self.device,
                &self.physical,
                MOST_SIMS * SIM_EMITTER_BYTES,
                BufferUsage::STORAGE_BUFFER,
            )?);
        }
        let frames = u32::try_from(FRAMES).unwrap_or(1);
        let pool = DescriptorPool::new(
            &self.device,
            frames,
            &[DescriptorPoolSize {
                descriptor_type: DescriptorType::STORAGE_BUFFER,
                descriptor_count: frames * 3,
            }],
        )?;
        let places: Vec<SurfacePoint> = map
            .effects
            .iter()
            .flat_map(|effect| &effect.emitters)
            .flat_map(|emitter| emitter.surface.iter().copied())
            .collect();
        let surfaces = surface_buffer(&self.device, &self.physical, &places)?;
        let mut sets = Vec::with_capacity(FRAMES);
        let kind = DescriptorType::STORAGE_BUFFER;
        for frame in &emitters {
            let set = pool.allocate(&self.sim_layout.set_layout)?;
            set.write_buffer(
                0,
                kind,
                &slots.buffer,
                0,
                u64::from(MOST_SLOTS) * SLOT_BYTES,
            );
            set.write_buffer(
                1,
                kind,
                &frame.buffer,
                0,
                (MOST_SIMS * SIM_EMITTER_BYTES) as u64,
            );
            let size = (places.len().max(1) * SURFACE_BYTES) as u64;
            set.write_buffer(2, kind, &surfaces.buffer, 0, size);
            sets.push(set);
        }
        let field_count = u32::try_from(map.fields.len() + 1).unwrap_or(1);
        let field_pool = DescriptorPool::new(
            &self.device,
            field_count,
            &[DescriptorPoolSize {
                descriptor_type: DescriptorType::SAMPLED_IMAGE,
                descriptor_count: field_count,
            }],
        )?;
        let mut field_sets = Vec::with_capacity(map.fields.len() + 1);
        let none = field_pool.allocate(&self.sim_layout.field_layout)?;
        let blank = self.upload_image(
            &[(1, 1, &[0, 0, 0, 0][..])],
            vulkane::safe::Format::R8G8B8A8_UNORM,
        )?;
        none.write_sampled_image(0, &blank.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        field_sets.push(none);
        // Emitters know fields by their place in the map, and one that wouldn't build isn't here.
        for def in &map.fields {
            let set = field_pool.allocate(&self.sim_layout.field_layout)?;
            match fields.iter().find(|field| field.name == def.name) {
                Some(field) => {
                    set.write_sampled_image(0, &field.shown.view, ImageLayout::GENERAL);
                }
                None => {
                    set.write_sampled_image(0, &blank.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                }
            }
            field_sets.push(set);
        }
        self.sim_blank = Some(blank);
        let (plain, motions) = self.sim_steps(&map.effect_motions)?;
        Ok(Some(GpuSims {
            slots,
            emitters,
            _surfaces: surfaces,
            sets,
            _pool: pool,
            field_sets,
            _field_pool: field_pool,
            plain,
            motions,
            codes: map.effect_motions.clone(),
            seen: None,
            ranges: Vec::new(),
            runs: std::array::from_fn(|_| Vec::new()),
        }))
    }

    /// The plain step, and one for each of `shaders`. One that's wrong steps as if it wasn't
    /// there, and says so.
    fn sim_steps(
        &mut self,
        shaders: &[Arc<ShaderCode>],
    ) -> Result<(ComputePipeline, Vec<ComputePipeline>), RenderError> {
        let samples = self.samples > 1;
        let plain = step_pipeline(&self.device, &self.sim_layout, "", samples)?;
        let mut motions = Vec::with_capacity(shaders.len());
        for shader in shaders {
            let made = step_pipeline(&self.device, &self.sim_layout, &shader.code, samples);
            motions.push(match made {
                Ok(pipeline) => pipeline,
                Err(problem) => {
                    self.shader_problems
                        .push(format!("{}: {}", shader.name, problem.0));
                    step_pipeline(&self.device, &self.sim_layout, "", samples)?
                }
            });
        }
        Ok((plain, motions))
    }

    /// Takes what's in `map` for effects, in place of what's on the GPU.
    pub(crate) fn reload_effects(&mut self, map: &Map) -> Result<(), RenderError> {
        self.wait_for_frame()?;
        self.device.wait_idle()?;
        let Some(mut gpu) = self.map.take() else {
            return Ok(());
        };
        let made = self.effects_of(map, &gpu.fields);
        let done = made.map(|(pictures, shaders, sims)| {
            gpu.particle_pictures = pictures;
            gpu.particle_shaders = shaders;
            gpu.particle_codes.clone_from(&map.effect_shaders);
            gpu.sims = sims;
        });
        self.map = Some(gpu);
        done
    }

    /// What effects have on the GPU, from `map`.
    fn effects_of(
        &mut self,
        map: &Map,
        fields: &[GpuField],
    ) -> Result<(ParticlePictures, Vec<ParticlePipelines>, Option<GpuSims>), RenderError> {
        let pictures = self.upload_particle_pictures(map)?;
        let shaders = self.upload_particle_shaders(&map.effect_shaders)?;
        let sims = self.upload_sims(map, fields)?;
        Ok((pictures, shaders, sims))
    }

    /// Makes what a level's particles have of their own again, for a scene with other samples.
    pub(super) fn remake_map_particles(&mut self) -> Result<(), RenderError> {
        let Some(mut map) = self.map.take() else {
            return Ok(());
        };
        let made = self.upload_particle_shaders(&map.particle_codes);
        let steps = map
            .sims
            .as_ref()
            .map(|sims| self.sim_steps(&sims.codes))
            .transpose();
        let done = match (made, steps) {
            (Ok(shaders), Ok(steps)) => {
                map.particle_shaders = shaders;
                if let (Some(sims), Some((plain, motions))) = (&mut map.sims, steps) {
                    sims.plain = plain;
                    sims.motions = motions;
                    sims.seen = None;
                }
                Ok(())
            }
            (Err(problem), _) | (_, Err(problem)) => Err(problem),
        };
        self.map = Some(map);
        done
    }

    /// Works out this frame's steps of `sims`, with `view` and its inverse for what hits things.
    pub(super) fn plan_sims(
        &mut self,
        sims: &[ParticleSim],
        (view, back): (&[f32; 16], &[f32; 16]),
        slot: usize,
    ) -> Result<(), RenderError> {
        let depth_ok = self
            .scene_target
            .as_ref()
            .is_some_and(|target| target.drawn.get());
        let Some(gpu) = self.map.as_mut().and_then(|map| map.sims.as_mut()) else {
            return Ok(());
        };

        gpu.ranges
            .retain(|range| sims.iter().any(|sim| sim.id == range.id));
        gpu.runs[slot].clear();
        // The depth a step reads is the last frame's, so it's that frame's view it wants.
        let drew = gpu.seen.replace((*view, *back));
        let (view, back) = drew
            .as_ref()
            .map_or((view, back), |seen| (&seen.0, &seen.1));
        let mut mapped = gpu.emitters[slot].memory.map()?;
        let target = mapped.as_slice_mut();
        for sim in sims.iter().take(MOST_SIMS) {
            let known = gpu.ranges.iter().position(|range| range.id == sim.id);
            let fresh = known.is_none();
            let at = match known {
                Some(at) => at,
                None => {
                    let Some(first) = free_run(&gpu.ranges, sim.count) else {
                        continue;
                    };
                    gpu.ranges.push(SimRange {
                        id: sim.id,
                        first,
                        count: sim.count,
                        head: 0,
                    });
                    gpu.ranges.sort_by_key(|range| range.first);
                    gpu.ranges
                        .iter()
                        .position(|range| range.id == sim.id)
                        .unwrap_or(0)
                }
            };
            let range = &mut gpu.ranges[at];
            let spawn = sim.spawn.min(range.count);
            let number = gpu.runs[slot].len();
            let bytes = sim_emitter_bytes(
                sim,
                (view, back),
                [range.first, range.count, range.head, spawn],
                [u32::from(fresh), u32::from(depth_ok)],
            );
            let place = number * SIM_EMITTER_BYTES;
            target[place..place + SIM_EMITTER_BYTES].copy_from_slice(&bytes);
            range.head = (range.head + spawn) % range.count.max(1);
            gpu.runs[slot].push(SimRun {
                emitter: u32::try_from(number).unwrap_or(0),
                count: range.count,
                shader: sim.shader,
                motion: sim.motion,
                field: sim.field,
            });
        }
        Ok(())
    }

    /// Records this frame's steps. What draws reads what they leave.
    pub(super) fn record_sims(&self, rec: &mut vulkane::safe::CommandBufferRecording<'_>) {
        let (Some(map), Some(target)) = (&self.map, &self.scene_target) else {
            return;
        };
        let Some(gpu) = &map.sims else {
            return;
        };
        let runs = &gpu.runs[self.slot];
        if runs.is_empty() {
            return;
        }
        // The last frame may still be reading slots, and drawing the depth a step reads.
        rec.memory_barrier(
            PipelineStage::ALL_COMMANDS,
            PipelineStage::COMPUTE_SHADER,
            AccessFlags::MEMORY_WRITE,
            AccessFlags::SHADER_READ | AccessFlags::SHADER_WRITE,
        );
        for run in runs {
            let own = (run.motion as usize).checked_sub(1);
            let pipeline = own.and_then(|at| gpu.motions.get(at)).unwrap_or(&gpu.plain);
            let field = gpu
                .field_sets
                .get(run.field as usize)
                .unwrap_or(&gpu.field_sets[0]);
            rec.bind_compute_pipeline(pipeline);
            rec.bind_compute_descriptor_sets(
                &self.sim_layout.layout,
                0,
                &[&gpu.sets[self.slot], &target.particle_set, field],
            );
            let mut push = [0; SIM_PUSH as usize];
            push[..4].copy_from_slice(&run.emitter.to_le_bytes());
            rec.push_constants(&self.sim_layout.layout, ShaderStageFlags::COMPUTE, 0, &push);
            rec.dispatch(run.count.div_ceil(GROUP), 1, 1);
        }
        // Past here the scene clears the depth steps have read, and what draws reads the slots.
        rec.memory_barrier(
            PipelineStage::COMPUTE_SHADER,
            PipelineStage::ALL_COMMANDS,
            AccessFlags::SHADER_WRITE,
            AccessFlags::SHADER_READ,
        );
    }

    /// What's in the slots of emitter `id`, for what checks it: each as a place and an age, then a
    /// way it's moving and how long it has.
    pub(crate) fn read_sim(&mut self, id: u64) -> Result<Vec<[f32; 8]>, RenderError> {
        self.wait_for_frame()?;
        self.device.wait_idle()?;
        let Some(gpu) = self.map.as_ref().and_then(|map| map.sims.as_ref()) else {
            return Ok(Vec::new());
        };
        let Some(range) = gpu.ranges.iter().find(|range| range.id == id).copied() else {
            return Ok(Vec::new());
        };
        let bytes = u64::from(range.count) * SLOT_BYTES;
        let usize_bytes = usize::try_from(bytes).unwrap_or(0);
        let mut staging = host_buffer(
            &self.device,
            &self.physical,
            usize_bytes,
            BufferUsage::TRANSFER_DST,
        )?;
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
                rec.copy_buffer(
                    &gpu.slots.buffer,
                    &staging.buffer,
                    &[BufferCopy {
                        src_offset: u64::from(range.first) * SLOT_BYTES,
                        dst_offset: 0,
                        size: bytes,
                    }],
                );
                Ok(())
            })?;
        let mapped = staging.memory.map()?;
        let raw = &mapped.as_slice()[..usize_bytes];
        Ok(raw
            .as_chunks::<32>()
            .0
            .iter()
            .map(|slot| {
                let mut out = [0.0; 8];
                for (number, chunk) in out.iter_mut().zip(slot.as_chunks::<4>().0) {
                    *number = f32::from_le_bytes(*chunk);
                }
                out
            })
            .collect())
    }
}

/// The first free run of `count` slots among `ranges`, which are lowest first.
fn free_run(ranges: &[SimRange], count: u32) -> Option<u32> {
    let mut first = 0;
    for range in ranges {
        if range.first - first >= count {
            return Some(first);
        }
        first = range.first + range.count;
    }
    (MOST_SLOTS - first >= count).then_some(first)
}
