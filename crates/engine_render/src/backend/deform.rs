//! Moving meshes: a compute pass moves their corners into plain vertices, and from there on
//! they're drawn like any other mesh (proposal 0140).

use engine_core::math::Vec3;
use engine_map::{Map, Rows, SharedMorph, SharedPart};
use engine_shaders::{Module, spirv};
use vulkane::safe::{
    AccessFlags2, BufferUsage, ComputePipeline, DescriptorPool, DescriptorPoolSize, DescriptorSet,
    DescriptorSetLayout, DescriptorSetLayoutBinding, DescriptorType, Device, PipelineLayout,
    PipelineStage2, ShaderModule, ShaderStageFlags,
};

use super::upload::host_buffer;
use super::{Backend, FRAMES, GpuBuffer, INDEX_TYPE_U32};
use crate::error::RenderError;
use crate::instances::{
    INSTANCE_BYTES, MovingDraw, PLACE_BYTES, UV_BYTES, ball_in_view, shared_places, shared_uvs,
    write_instance,
};
use crate::world::{InstanceLight, OWN_LIGHT_STYLE};

/// Corners in a frame's moved vertices. Past this, a mesh draws at rest.
const MOST_MOVED_CORNERS: usize = 1 << 18;

/// Joints in a frame's palette. Past this, a mesh draws at rest.
const MOST_JOINTS: usize = 1 << 14;

/// Moving meshes in a frame.
const MOST_MOVING: usize = 1024;

/// Bytes in a joint's three rows.
const JOINT_BYTES: usize = 48;

/// Morph weights in a frame's list. Past this, a mesh draws with none.
const MOST_WEIGHTS: usize = 1 << 14;

/// Bytes in what the shader's told of a mesh.
const JOB_BYTES: usize = 24;

/// Workgroups in a frame: as many as its corners fill, and a part-filled one for each mesh.
const MOST_GROUPS: usize = MOST_MOVED_CORNERS / WORKGROUP as usize + MOST_MOVING;

/// A float as a half float's bits, which is what morphs are kept in. They're small offsets, so
/// that's plenty, at a quarter of what's kept for each.
#[must_use]
pub fn half_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = u16::try_from((bits >> 16) & 0x8000).unwrap_or(0);
    let exponent = i32::try_from((bits >> 23) & 0xff).unwrap_or(0) - 127 + 15;
    let fraction = bits & 0x007f_ffff;
    if exponent >= 31 {
        // Too big for a half, so as big as there is.
        return sign | 0x7bff;
    }
    if exponent <= 0 {
        if exponent < -10 {
            return sign;
        }
        // Too small to have an exponent: the fraction shifts down, with its hidden bit.
        let shift = u32::try_from(14 - exponent).unwrap_or(24);
        let whole = fraction | 0x0080_0000;
        let rounded = (whole + (1 << (shift - 1))) >> shift;
        return sign | u16::try_from(rounded).unwrap_or(0);
    }
    let exponent = u32::try_from(exponent).unwrap_or(0);
    // Rounded, and a carry out of the fraction goes on into the exponent, as it should.
    let rounded = ((exponent << 10) | (fraction >> 13)) + ((fraction >> 12) & 1);
    sign | u16::try_from(rounded).unwrap_or(0x7bff)
}

/// `morph` as four words.
fn morph_words(morph: &SharedMorph) -> [u32; 4] {
    let pair = |low: f32, high: f32| u32::from(half_bits(low)) | (u32::from(half_bits(high)) << 16);
    let [x, y, z] = morph.position;
    let [nx, ny, nz] = morph.normal;
    [
        u32::from(morph.target),
        pair(x, y),
        pair(z, nx),
        pair(ny, nz),
    ]
}

/// How many corners a workgroup does, which is in the shader too.
const WORKGROUP: u32 = 64;

/// What moves meshes, which lasts as long as the device.
pub(super) struct Deform {
    /// The pipeline.
    pipeline: ComputePipeline,
    /// Its layout.
    layout: PipelineLayout,
    /// Its set's layout.
    set_layout: DescriptorSetLayout,
}

impl Deform {
    /// Builds it.
    pub(super) fn new(device: &Device) -> Result<Self, RenderError> {
        let code = spirv(&Module::Deform).map_err(RenderError)?;
        let shader = ShaderModule::from_spirv(device, &code)?;
        let binding = |binding| DescriptorSetLayoutBinding {
            binding,
            descriptor_type: DescriptorType::STORAGE_BUFFER,
            descriptor_count: 1,
            stage_flags: ShaderStageFlags::COMPUTE,
        };
        let set_layout =
            DescriptorSetLayout::new(device, &[0, 1, 2, 3, 4, 5, 6, 7, 8].map(binding))?;
        let layout = PipelineLayout::new(device, &[&set_layout])?;
        let pipeline = ComputePipeline::new(device, &layout, &shader, "cs_deform")?;
        Ok(Self {
            pipeline,
            layout,
            set_layout,
        })
    }
}

/// A moving mesh on the GPU.
pub(super) struct GpuMovingMesh {
    /// Where its corners start among all of them.
    first_corner: u32,
    /// How many it has.
    corners: u32,
    /// Its triangles.
    pub(super) indices: GpuBuffer,
    /// Its parts, by material.
    pub(super) parts: Vec<SharedPart>,
    /// Those that cast shadows.
    pub(super) shadow_parts: Vec<SharedPart>,
}

/// One mesh to move and draw this frame.
#[derive(Clone, Copy, Debug)]
pub(super) struct MovingJob {
    /// Which mesh.
    pub(super) mesh: u32,
    /// Where its moved corners went, or `None` if it's drawn at rest.
    moved_to: Option<u32>,
    /// Its place in the instance buffer.
    pub(super) instance: u32,
    /// What places it, column by column, for shadow maps.
    pub(super) placed: [f32; 16],
    /// Its middle.
    pub(super) center: Vec3,
    /// How far it reaches from there.
    pub(super) radius: f32,
}

/// What's kept for one frame in flight.
struct MovingFrame {
    /// The joints.
    palette: GpuBuffer,
    /// The morph weights.
    weights: GpuBuffer,
    /// The moved corners.
    moved: GpuBuffer,
    /// What the shader's told of each mesh.
    work: GpuBuffer,
    /// Which mesh each workgroup does.
    groups: GpuBuffer,
    /// How many workgroups there are this frame.
    group_count: u32,
    /// Where each mesh is, as instances.
    instances: GpuBuffer,
    /// The set reading and writing those.
    set: DescriptorSet,
    /// What's to do.
    jobs: Vec<MovingJob>,
}

/// The moving meshes of a map, on the GPU.
pub(super) struct GpuMoving {
    /// Everyone's corners at rest.
    corners: GpuBuffer,
    /// Everyone's UVs, which don't move.
    uvs: GpuBuffer,
    /// Everyone's joints and weights. Kept alive for the sets.
    _skin: GpuBuffer,
    /// Where each corner's morphs are. Kept alive for the sets.
    _morph_heads: GpuBuffer,
    /// Everyone's morphs. Kept alive for the sets.
    _morphs: GpuBuffer,
    /// Each mesh.
    pub(super) meshes: Vec<GpuMovingMesh>,
    /// Each frame in flight's.
    frames: Vec<MovingFrame>,
    /// Kept alive for the sets.
    _pool: DescriptorPool,
}

/// The rows of `rows`, by columns.
fn columns_of(rows: &Rows) -> [f32; 16] {
    let [x, y, z] = rows;
    [
        x[0], y[0], z[0], 0.0, x[1], y[1], z[1], 0.0, x[2], y[2], z[2], 0.0, x[3], y[3], z[3], 1.0,
    ]
}

impl GpuMoving {
    /// Where `job`'s corners are, to draw it.
    pub(super) fn places_of(&self, job: &MovingJob, slot: usize) -> (&GpuBuffer, u64) {
        let place = PLACE_BYTES as u64;
        match job.moved_to {
            Some(first) => (&self.frames[slot].moved, u64::from(first) * place),
            None => (&self.corners, u64::from(self.first_corner(job)) * place),
        }
    }

    /// Where `job`'s mesh starts among everyone's corners.
    fn first_corner(&self, job: &MovingJob) -> u32 {
        self.meshes
            .get(job.mesh as usize)
            .map_or(0, |mesh| mesh.first_corner)
    }

    /// This frame's jobs.
    pub(super) fn jobs(&self, slot: usize) -> &[MovingJob] {
        &self.frames[slot].jobs
    }

    /// This frame's instances.
    pub(super) fn instances(&self, slot: usize) -> &GpuBuffer {
        &self.frames[slot].instances
    }

    /// Writes this frame's joints and instances, and works out what's to do. Says if anything
    /// had to be drawn at rest for want of room.
    pub(super) fn prepare(
        &mut self,
        slot: usize,
        draws: &[MovingDraw],
        (palette, morph_weights): (&[Rows], &[f32]),
        planes: &[[f32; 4]; 6],
    ) -> Result<bool, RenderError> {
        let meshes = &self.meshes;
        let frame = &mut self.frames[slot];
        frame.jobs.clear();
        let mut over = false;
        let mut used_corners = 0_usize;
        let mut used_joints = 0_usize;
        let mut used_weights = 0_usize;
        let mut used_groups = 0_usize;
        let mut moved_jobs = 0_usize;
        let mut work = frame.work.memory.map()?;
        let (work, _) = work.as_slice_mut().as_chunks_mut::<JOB_BYTES>();
        let mut groups = frame.groups.memory.map()?;
        let (groups, _) = groups.as_slice_mut().as_chunks_mut::<4>();
        let mut weights = frame.weights.memory.map()?;
        let (weights, _) = weights.as_slice_mut().as_chunks_mut::<4>();
        let mut joints = frame.palette.memory.map()?;
        let joints = joints.as_slice_mut();
        let mut instances = frame.instances.memory.map()?;
        let mut slots = instances
            .as_slice_mut()
            .as_chunks_mut::<INSTANCE_BYTES>()
            .0
            .iter_mut();
        for draw in draws {
            let Some(mesh) = meshes.get(draw.mesh as usize) else {
                continue;
            };
            if !ball_in_view(planes, draw.center, draw.radius) {
                continue;
            }
            let Some(instance) = slots.next() else {
                over = true;
                break;
            };
            let first = draw.first_joint as usize;
            let own = palette.get(first..first + draw.joint_count as usize);
            let fits = used_corners + mesh.corners as usize <= MOST_MOVED_CORNERS
                && used_joints + draw.joint_count as usize <= MOST_JOINTS;
            let mut moved_to = None;
            let first_joint = u32::try_from(used_joints).unwrap_or(0);
            if let (true, Some(own)) = (fits, own) {
                for (rows, target) in own.iter().zip(
                    joints[used_joints * JOINT_BYTES..]
                        .as_chunks_mut::<JOINT_BYTES>()
                        .0,
                ) {
                    let numbers = rows.iter().flatten();
                    for (chunk, number) in target.as_chunks_mut::<4>().0.iter_mut().zip(numbers) {
                        *chunk = number.to_le_bytes();
                    }
                }
                moved_to = Some(u32::try_from(used_corners).unwrap_or(0));
                used_corners += mesh.corners as usize;
                used_joints += own.len();
            } else {
                over = true;
            }
            let first = draw.first_weight as usize;
            let own = morph_weights.get(first..first + draw.weight_count as usize);
            let first_weight = u32::try_from(used_weights).unwrap_or(0);
            let mut weight_count = 0;
            if let Some(own) = own.filter(|own| used_weights + own.len() <= MOST_WEIGHTS) {
                for (slot, weight) in weights[used_weights..].iter_mut().zip(own) {
                    *slot = weight.to_le_bytes();
                }
                used_weights += own.len();
                weight_count = draw.weight_count;
            }
            if let Some(first_out) = moved_to {
                let words = [
                    mesh.first_corner,
                    mesh.corners,
                    first_out,
                    first_joint,
                    first_weight,
                    weight_count,
                ];
                let row = work[moved_jobs].as_chunks_mut::<4>().0.iter_mut();
                for (chunk, word) in row.zip(words) {
                    *chunk = word.to_le_bytes();
                }
                let job = u32::try_from(moved_jobs).unwrap_or(0);
                for group in 0..mesh.corners.div_ceil(WORKGROUP) {
                    groups[used_groups] = (job | group << 10).to_le_bytes();
                    used_groups += 1;
                }
                moved_jobs += 1;
            }
            let [red, green, blue] = draw.light;
            let light = InstanceLight {
                uv: [red, green],
                step: blue,
                styles: [OWN_LIGHT_STYLE, u8::MAX, u8::MAX, u8::MAX],
            };
            write_instance(&draw.rows, &light, instance.as_mut_slice());
            frame.jobs.push(MovingJob {
                mesh: draw.mesh,
                moved_to,
                instance: u32::try_from(frame.jobs.len()).unwrap_or(0),
                placed: columns_of(&draw.rows),
                center: draw.center,
                radius: draw.radius,
            });
        }
        frame.group_count = u32::try_from(used_groups).unwrap_or(0);
        Ok(over)
    }
}

impl Backend {
    /// Puts `map`'s moving meshes on the GPU, if it has any.
    pub(super) fn upload_moving(&self, map: &Map) -> Result<Option<GpuMoving>, RenderError> {
        if map.moving_meshes.is_empty() {
            return Ok(None);
        }
        let mut corner_bytes = Vec::new();
        let mut uv_bytes = Vec::new();
        let mut skin_bytes = Vec::new();
        let mut head_bytes = Vec::new();
        let mut morph_bytes = Vec::new();
        let mut meshes = Vec::with_capacity(map.moving_meshes.len());
        let mut first_corner = 0_u32;
        for moving in &map.moving_meshes {
            let shared = &moving.mesh;
            corner_bytes.extend(shared_places(shared));
            uv_bytes.extend(shared_uvs(shared));
            for corner in 0..shared.corners.len() {
                // A mesh with no skin stays with its first joint, which is at rest, or nothing.
                let joints = shared.joints.get(corner).copied().unwrap_or([0; 4]);
                let weights = shared
                    .weights
                    .get(corner)
                    .copied()
                    .unwrap_or([255, 0, 0, 0]);
                skin_bytes.extend_from_slice(&joints);
                skin_bytes.extend_from_slice(&weights);
                // Heads say where in everyone's morphs, so this mesh's are moved along.
                let first = u32::try_from(morph_bytes.len() / 16).unwrap_or(0);
                let [own, count] = shared.morph_heads.get(corner).copied().unwrap_or([0, 0]);
                head_bytes.extend_from_slice(&(first + own).to_le_bytes());
                head_bytes.extend_from_slice(&count.to_le_bytes());
            }
            for morph in &shared.morphs {
                morph_bytes.extend(
                    morph_words(morph)
                        .iter()
                        .flat_map(|word| word.to_le_bytes()),
                );
            }
            let index_bytes: Vec<u8> = shared
                .indices
                .iter()
                .flat_map(|i| i.to_le_bytes())
                .collect();
            let parts = shared.levels.first().cloned().unwrap_or_default();
            meshes.push(GpuMovingMesh {
                first_corner,
                corners: u32::try_from(shared.corners.len()).unwrap_or(0),
                indices: self.device_buffer(&index_bytes, BufferUsage::INDEX_BUFFER)?,
                shadow_parts: parts
                    .iter()
                    .filter(|part| super::upload::casts_shadow(map, part.texture))
                    .copied()
                    .collect(),
                parts,
            });
            first_corner += u32::try_from(shared.corners.len()).unwrap_or(0);
        }
        // Read by the compute pass, and drawn from when a mesh is at rest.
        let corners = self.device_buffer(
            &corner_bytes,
            BufferUsage::STORAGE_BUFFER | BufferUsage::VERTEX_BUFFER,
        )?;
        let uvs = self.device_buffer(&uv_bytes, BufferUsage::VERTEX_BUFFER)?;
        let skin = self.device_buffer(&skin_bytes, BufferUsage::STORAGE_BUFFER)?;
        let heads = self.device_buffer(&head_bytes, BufferUsage::STORAGE_BUFFER)?;
        let morphs = self.device_buffer(&morph_bytes, BufferUsage::STORAGE_BUFFER)?;
        let sets = u32::try_from(FRAMES).unwrap_or(2);
        let pool = DescriptorPool::new(
            &self.device,
            sets,
            &[DescriptorPoolSize {
                descriptor_type: DescriptorType::STORAGE_BUFFER,
                descriptor_count: sets * 9,
            }],
        )?;
        let mut frames = Vec::with_capacity(FRAMES);
        let moved_bytes = MOST_MOVED_CORNERS * PLACE_BYTES;
        for _ in 0..FRAMES {
            let palette = host_buffer(
                &self.device,
                &self.physical,
                MOST_JOINTS * JOINT_BYTES,
                BufferUsage::STORAGE_BUFFER,
            )?;
            let weights = host_buffer(
                &self.device,
                &self.physical,
                MOST_WEIGHTS * 4,
                BufferUsage::STORAGE_BUFFER,
            )?;
            let moved = self.scratch_buffer(
                moved_bytes,
                BufferUsage::STORAGE_BUFFER | BufferUsage::VERTEX_BUFFER,
            )?;
            let instances = host_buffer(
                &self.device,
                &self.physical,
                MOST_MOVING * INSTANCE_BYTES,
                BufferUsage::VERTEX_BUFFER,
            )?;
            let work = host_buffer(
                &self.device,
                &self.physical,
                MOST_MOVING * JOB_BYTES,
                BufferUsage::STORAGE_BUFFER,
            )?;
            let groups = host_buffer(
                &self.device,
                &self.physical,
                MOST_GROUPS * 4,
                BufferUsage::STORAGE_BUFFER,
            )?;
            let set = pool.allocate(&self.deform.set_layout)?;
            let storage = DescriptorType::STORAGE_BUFFER;
            set.write_buffer(
                0,
                storage,
                &corners.buffer,
                0,
                corner_bytes.len().max(4) as u64,
            );
            set.write_buffer(1, storage, &skin.buffer, 0, skin_bytes.len().max(4) as u64);
            set.write_buffer(
                2,
                storage,
                &palette.buffer,
                0,
                (MOST_JOINTS * JOINT_BYTES) as u64,
            );
            set.write_buffer(3, storage, &moved.buffer, 0, moved_bytes as u64);
            set.write_buffer(4, storage, &heads.buffer, 0, head_bytes.len().max(4) as u64);
            set.write_buffer(
                5,
                storage,
                &morphs.buffer,
                0,
                morph_bytes.len().max(16) as u64,
            );
            set.write_buffer(6, storage, &weights.buffer, 0, (MOST_WEIGHTS * 4) as u64);
            set.write_buffer(
                7,
                storage,
                &work.buffer,
                0,
                (MOST_MOVING * JOB_BYTES) as u64,
            );
            set.write_buffer(8, storage, &groups.buffer, 0, (MOST_GROUPS * 4) as u64);
            frames.push(MovingFrame {
                palette,
                weights,
                moved,
                work,
                groups,
                group_count: 0,
                instances,
                set,
                jobs: Vec::new(),
            });
        }
        Ok(Some(GpuMoving {
            corners,
            uvs,
            _skin: skin,
            _morph_heads: heads,
            _morphs: morphs,
            meshes,
            frames,
            _pool: pool,
        }))
    }

    /// Records moving this frame's meshes. What draws them waits for this.
    pub(super) fn record_deform(&self, rec: &mut vulkane::safe::CommandBufferRecording<'_>) {
        let Some(moving) = self.map.as_ref().and_then(|map| map.moving.as_ref()) else {
            return;
        };
        let frame = &moving.frames[self.slot];
        if frame.group_count == 0 {
            return;
        }
        rec.bind_compute_pipeline(&self.deform.pipeline);
        rec.bind_compute_descriptor_sets(&self.deform.layout, 0, &[&frame.set]);
        rec.dispatch(frame.group_count, 1, 1);
        // Vulkan 1.3 is required, so this is always there, and if it weren't the device
        // wouldn't have been picked.
        let _barrier = rec.memory_barrier2(
            PipelineStage2::COMPUTE_SHADER,
            PipelineStage2::VERTEX_ATTRIBUTE_INPUT,
            AccessFlags2::SHADER_STORAGE_WRITE,
            AccessFlags2::VERTEX_ATTRIBUTE_READ,
        );
    }
}

/// How a moving mesh's parts draw: bound here, so that a shadow map and the scene agree.
pub(super) fn bind_moving(
    rec: &mut vulkane::safe::CommandBufferRecording<'_>,
    moving: &GpuMoving,
    job: &MovingJob,
    slot: usize,
    instances: bool,
) -> Option<()> {
    let mesh = moving.meshes.get(job.mesh as usize)?;
    let (corners, offset) = moving.places_of(job, slot);
    if instances {
        let uvs = u64::from(mesh.first_corner) * UV_BYTES as u64;
        rec.bind_vertex_buffers(
            0,
            &[
                (&corners.buffer, offset),
                (&moving.uvs.buffer, uvs),
                (&moving.instances(slot).buffer, 0),
            ],
        );
    } else {
        rec.bind_vertex_buffers(0, &[(&corners.buffer, offset)]);
    }
    rec.bind_index_buffer(&mesh.indices.buffer, 0, INDEX_TYPE_U32);
    Some(())
}
