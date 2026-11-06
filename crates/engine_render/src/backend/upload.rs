//! Getting what's drawn onto the GPU.

use std::sync::Arc;

use engine_core::canvas::Quad;
use engine_core::math::Vec3;
use engine_map::{
    AliasModel, BOUNCE_STYLE, Fog, Map, Pool, ProbeGrid, RayStencil, STENCIL_GRID, Sky, Sprite,
    TextureKind,
};
use vulkane::safe::{
    AccessFlags, Buffer, BufferCopy, BufferCreateInfo, BufferImageCopy, BufferUsage,
    DescriptorPool, DescriptorPoolSize, DescriptorSet, DescriptorType, Device, DeviceMemory,
    Format, Image, Image2dCreateInfo, ImageBarrier, ImageLayout, ImageUsage, ImageView,
    MemoryPropertyFlags, PhysicalDevice, PipelineStage,
};

use super::shadow::{ShadowRange, shadow_figures};
use super::{
    Backend, DEPTH_FORMAT, EXTRA_FLOATS, FRAME_SIZE, GpuAlias, GpuBuffer, GpuImage, GpuMap,
    GpuPicture, GpuSprite, LIVE_BYTES, MAX_POOLS, POST_BYTES, SKY_BYTES, Scene,
};
use crate::alias::{pose_bytes, uv_bytes};
use crate::canvas::{VERTEX_FLOATS, canvas_vertices};
use crate::error::RenderError;
use crate::live::live_bytes;
use crate::material::{
    MATERIAL_PARAMS_BYTES, MATERIAL_PARAMS_STRIDE, material_params as material_params_of,
};
use crate::matrix::{inverse, view_projection};
use crate::renderer::{CapsuleDraw, PostSettings, View};
use crate::sprite::{LightDraw, MAX_LIGHT_DRAWS, PARTICLE_BYTES, write_particle};
use crate::world::{BatchKind, uses_material};
use crate::world::{TileImage, VertexPatch, shadow_groups};
use crate::world::{Vertex, WorldMesh};

/// A buffer the CPU can write.
/// Copies a square `size` across from `offset` in `staging` to `corner` of
/// `atlas`, which is read before and after.
fn copy_into_atlas(
    rec: &mut vulkane::safe::CommandBufferRecording<'_>,
    staging: &GpuBuffer,
    atlas: &GpuImage,
    offset: u64,
    size: u32,
    corner: [u32; 2],
) {
    rec.image_barrier(
        PipelineStage::ALL_GRAPHICS,
        PipelineStage::TRANSFER,
        ImageBarrier::color(
            &atlas.image,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ImageLayout::TRANSFER_DST_OPTIMAL,
            AccessFlags::SHADER_READ,
            AccessFlags::TRANSFER_WRITE,
        ),
    );
    let mut region = BufferImageCopy::full_2d(size, size);
    region.buffer_offset = offset;
    region.image_offset = [
        i32::try_from(corner[0]).unwrap_or(0),
        i32::try_from(corner[1]).unwrap_or(0),
        0,
    ];
    rec.copy_buffer_to_image(
        &staging.buffer,
        &atlas.image,
        ImageLayout::TRANSFER_DST_OPTIMAL,
        &[region],
    );
    rec.image_barrier(
        PipelineStage::TRANSFER,
        PipelineStage::FRAGMENT_SHADER,
        ImageBarrier::color(
            &atlas.image,
            ImageLayout::TRANSFER_DST_OPTIMAL,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            AccessFlags::TRANSFER_WRITE,
            AccessFlags::SHADER_READ,
        ),
    );
}

pub(super) fn host_buffer(
    device: &Device,
    physical: &PhysicalDevice,
    size: usize,
    usage: BufferUsage,
) -> Result<GpuBuffer, RenderError> {
    let (buffer, memory) = Buffer::new_bound(
        device,
        physical,
        BufferCreateInfo {
            size: size.max(4) as u64,
            usage,
        },
        MemoryPropertyFlags::HOST_VISIBLE | MemoryPropertyFlags::HOST_COHERENT,
    )?;
    Ok(GpuBuffer { buffer, memory })
}

/// An image in device memory.
pub(super) fn device_image(
    device: &Device,
    physical: &PhysicalDevice,
    image: Image,
    depth: bool,
) -> Result<GpuImage, RenderError> {
    let requirements = image.memory_requirements();
    let memory_type = physical
        .find_memory_type(
            requirements.memory_type_bits,
            MemoryPropertyFlags::DEVICE_LOCAL,
        )
        .ok_or_else(|| RenderError("no memory for an image".to_owned()))?;
    let memory = DeviceMemory::allocate(device, requirements.size, memory_type)?;
    image.bind_memory(&memory, 0)?;
    let view = if depth {
        ImageView::new_2d_depth(&image)?
    } else {
        ImageView::new_2d_color(&image)?
    };
    Ok(GpuImage {
        view,
        image,
        _memory: memory,
    })
}

/// A depth image of `size`.
pub(super) fn depth_image(
    device: &Device,
    physical: &PhysicalDevice,
    size: (u32, u32),
) -> Result<GpuImage, RenderError> {
    let image = Image::new_2d(
        device,
        Image2dCreateInfo {
            format: DEPTH_FORMAT,
            width: size.0,
            height: size.1,
            usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
        },
    )?;
    device_image(device, physical, image, true)
}

/// `view`'s uniforms, with `lights`.
#[expect(
    clippy::cast_precision_loss,
    reason = "there are at most a few dozen lights"
)]
pub(super) fn frame_bytes(
    view: &View,
    aspect: f32,
    lights: &[LightDraw],
    sky: Option<&Sky>,
    fog: Option<&Fog>,
    post: &PostSettings,
    extra: &FrameExtras<'_>,
) -> Vec<u8> {
    let lights = lights.get(..MAX_LIGHT_DRAWS).unwrap_or(lights);
    let matrix = view_projection(
        view.origin,
        [view.yaw, view.pitch, view.roll],
        view.fov,
        aspect,
    );
    let eye = [view.origin.x, view.origin.y, view.origin.z, 1.0];
    let params = [view.time, aspect, lights.len() as f32, view.gamma];
    let mut positions = [0.0; MAX_LIGHT_DRAWS * 4];
    let mut cuts = [0.0; MAX_LIGHT_DRAWS];
    for ((light, position), cut) in lights
        .iter()
        .zip(positions.as_chunks_mut::<4>().0)
        .zip(&mut cuts)
    {
        *position = [light.origin.x, light.origin.y, light.origin.z, light.radius];
        *cut = light.min_light;
    }
    let mut styles = view.styles;
    if let Some(bounce) = styles.get_mut(usize::from(BOUNCE_STYLE)) {
        *bounce = 1.0;
    }
    let mut out = Vec::with_capacity(FRAME_SIZE);
    for value in matrix
        .iter()
        .chain(&eye)
        .chain(&params)
        .chain(&styles)
        .chain(&positions)
        .chain(&cuts)
        .chain(&sky_values(sky))
        .chain(&post_values(&matrix, post, fog))
        .chain(&extra_values(extra, post))
    {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// What a frame's stencils and capsules are.
#[derive(Clone, Copy)]
pub(super) struct FrameExtras<'a> {
    /// The stencils.
    stencils: &'a [RayStencil],
    /// Where their lights are now.
    lights: &'a [Vec3],
    /// How open their cells are.
    masks: &'a [[f32; STENCIL_GRID * STENCIL_GRID]],
    /// The characters shading light.
    capsules: &'a [CapsuleDraw],
    /// Bodies of liquid.
    pools: &'a [Pool],
    /// How many samples a pixel the scene has.
    samples: u32,
    /// Where the view is, to choose among them.
    eye: Vec3,
    /// Whether the view is under the open sky, so that mirrors can show it.
    under_sky: bool,
    /// The baked light in the air.
    probes: Option<&'a ProbeGrid>,
}

/// The stencil and capsule part of the frame uniforms.
fn extra_values(extra: &FrameExtras<'_>, post: &PostSettings) -> Vec<f32> {
    let FrameExtras {
        stencils,
        lights,
        masks,
        capsules,
        ..
    } = *extra;
    let mut out = vec![0.0; EXTRA_FLOATS];
    let count = if post.rays == 0 {
        0
    } else {
        stencils.len().min(4)
    };
    for (index, stencil) in stencils.iter().take(4).enumerate() {
        let at = index * 4;
        out[at..at + 4].copy_from_slice(&[
            stencil.corner.x,
            stencil.corner.y,
            stencil.corner.z,
            stencil.strength,
        ]);
        let at = 16 + index * 4;
        out[at..at + 3].copy_from_slice(&[stencil.across.x, stencil.across.y, stencil.across.z]);
        let at = 32 + index * 4;
        out[at..at + 4].copy_from_slice(&[
            stencil.down.x,
            stencil.down.y,
            stencil.down.z,
            stencil.reach,
        ]);
        let at = 48 + index * 4;
        let light = lights.get(index).copied().unwrap_or(stencil.light);
        out[at..at + 4].copy_from_slice(&[
            light.x,
            light.y,
            light.z,
            f32::from(u8::from(stencil.sun)),
        ]);
        let at = 68 + index * 256;
        match masks.get(index) {
            Some(mask) => out[at..at + 256].copy_from_slice(mask),
            None => out[at..at + 256].fill(1.0),
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "four at most")]
    let stencil_count = count as f32;
    out[64] = stencil_count;
    // Steps along the view, and how blurred the stencil is read.
    let (steps, blur) = match post.rays {
        1 => (8.0, 8.0),
        2 => (12.0, 3.0),
        _ => (24.0, 1.5),
    };
    out[65] = steps;
    out[66] = blur;
    #[expect(clippy::cast_precision_loss, reason = "eight at most")]
    let samples = extra.samples as f32;
    out[67] = samples;
    let base = 68 + 1024;
    let shown = capsules.len().min(16);
    for (index, capsule) in capsules.iter().take(16).enumerate() {
        let at = base + index * 4;
        out[at..at + 4].copy_from_slice(&[
            capsule.origin.x,
            capsule.origin.y,
            capsule.origin.z,
            capsule.radius,
        ]);
        let at = base + 64 + index * 4;
        out[at..at + 4].copy_from_slice(&[
            capsule.toward_light.x,
            capsule.toward_light.y,
            capsule.toward_light.z,
            capsule.strength,
        ]);
    }
    #[expect(clippy::cast_precision_loss, reason = "sixteen at most")]
    let capsule_count = shown as f32;
    out[base + 128] = capsule_count;
    let base = base + 132;
    let mut pools: Vec<&Pool> = extra.pools.iter().collect();
    let distance = |pool: &Pool| (((pool.low + pool.high) * 0.5) - extra.eye).length();
    pools.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
    pools.truncate(MAX_POOLS);
    for (index, pool) in pools.iter().enumerate() {
        let at = base + index * 4;
        out[at..at + 4].copy_from_slice(&[
            pool.low.x,
            pool.low.y,
            pool.low.z,
            pool.look.absorption,
        ]);
        let at = base + MAX_POOLS * 4 + index * 4;
        out[at..at + 4].copy_from_slice(&[
            pool.high.x,
            pool.high.y,
            pool.high.z,
            pool.look.caustics,
        ]);
        let at = base + MAX_POOLS * 8 + index * 4;
        out[at..at + 3].copy_from_slice(&pool.look.color);
    }
    let base = base + MAX_POOLS * 12;
    #[expect(clippy::cast_precision_loss, reason = "eight at most")]
    let pool_count = pools.len() as f32;
    out[base] = pool_count;
    out[base + 1] = post.detail_distance;
    out[base + 2] = f32::from(u8::from(extra.under_sky));
    if let Some(probes) = extra.probes {
        out[base + 4..base + 8].copy_from_slice(&[
            probes.origin.x,
            probes.origin.y,
            probes.origin.z,
            probes.spacing,
        ]);
        #[expect(clippy::cast_precision_loss, reason = "grid sizes are small")]
        let size = probes.size.map(|axis| axis as f32);
        out[base + 8..base + 11].copy_from_slice(&size);
        out[base + 11] = f32::from(u8::from(post.fog_grid));
    }
    out
}

/// The final pass's part of the frame uniforms.
fn post_values(
    matrix: &[f32; 16],
    post: &PostSettings,
    fog: Option<&Fog>,
) -> [f32; POST_BYTES / 4] {
    let mut out = [0.0; POST_BYTES / 4];
    out[..16].copy_from_slice(&inverse(matrix).unwrap_or([0.0; 16]));
    if let Some(fog) = fog {
        out[16..19].copy_from_slice(&fog.color);
        out[19] = fog.density;
        out[20] = fog.height;
        out[21] = fog.falloff;
    }
    out[24..27].copy_from_slice(&post.outline_color);
    out[27] = f32::from(post.outline);
    out[28] = f32::from(post.edge_smoothing);
    out[29] = post.outline_width;
    out[30] = post.outline_fade;
    out[31] = post.rounded_edges;
    out
}

/// The sky's part of the frame uniforms.
fn sky_values(sky: Option<&Sky>) -> [f32; SKY_BYTES / 4] {
    let mut out = [0.0; SKY_BYTES / 4];
    let Some(sky) = sky else {
        return out;
    };
    out[..3].copy_from_slice(&sky.zenith);
    out[3] = sky.horizon_height;
    out[4..7].copy_from_slice(&sky.horizon);
    out[8..11].copy_from_slice(&sky.ground);
    out[12..15].copy_from_slice(&[
        sky.sun_direction.x,
        sky.sun_direction.y,
        sky.sun_direction.z,
    ]);
    out[15] = sky.sun_size;
    out[16..19].copy_from_slice(&sky.sun_color);
    if let Some(clouds) = &sky.clouds {
        out[20..23].copy_from_slice(&clouds.color);
        out[23] = clouds.coverage;
        out[24..26].copy_from_slice(&clouds.speed);
        out[26] = clouds.scale;
        out[27] = if clouds.image.is_some() { 2.0 } else { 1.0 };
    }
    out
}

impl Backend {
    /// Uploads a flat picture of RGBA pixels, and returns its number.
    pub(crate) fn add_picture(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<u32, RenderError> {
        let image = self.upload_image(&[(width, height, rgba)], Format::R8G8B8A8_UNORM)?;
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
        let canvas = self
            .canvas
            .as_mut()
            .ok_or_else(|| RenderError("no canvas".to_owned()))?;
        let set = pool.allocate(&canvas.set_layout)?;
        set.write_sampled_image(0, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        set.write_sampler(1, &canvas.sampler);
        canvas.pictures.push(GpuPicture {
            set,
            _pool: pool,
            _image: image,
        });
        Ok(u32::try_from(canvas.pictures.len() - 1).unwrap_or(u32::MAX))
    }

    /// Writes this frame's canvas.
    pub(super) fn upload_canvas(
        &mut self,
        quads: &[Quad],
        size: (u32, u32),
    ) -> Result<(), RenderError> {
        let Some(canvas) = &mut self.canvas else {
            return Ok(());
        };
        let (floats, batches) = canvas_vertices(quads, size);
        let mut mapped = canvas.vertices.memory.map()?;
        let (target, _remainder) = mapped.as_slice_mut().as_chunks_mut::<4>();
        let ui_first = floats.len() / VERTEX_FLOATS;
        for (value, chunk) in floats.iter().chain(&canvas.ui.vertices).zip(target) {
            *chunk = value.to_le_bytes();
        }
        drop(mapped);
        canvas.batches = batches;
        canvas.ui_first = u32::try_from(ui_first).unwrap_or(0);
        Ok(())
    }

    /// Uploads `pictures`, as a width, a height and pixels, each with a set that
    /// draws it.
    fn upload_textures(
        &self,
        pictures: &[(u32, u32, &[u8])],
    ) -> Result<(DescriptorPool, Vec<DescriptorSet>, Vec<GpuImage>), RenderError> {
        let sets = u32::try_from(pictures.len().max(1)).unwrap_or(u32::MAX);
        let pool = DescriptorPool::new(
            &self.device,
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
        let mut images = Vec::with_capacity(pictures.len());
        let mut image_sets = Vec::with_capacity(pictures.len());
        for picture in pictures {
            let image = self.upload_image(&[*picture], Format::R8G8B8A8_UNORM)?;
            let set = pool.allocate(&self.texture_layout)?;
            set.write_sampled_image(0, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            set.write_sampler(1, &self.texture_sampler);
            image_sets.push(set);
            images.push(image);
        }
        Ok((pool, image_sets, images))
    }

    /// Uploads a model, and returns its number.
    pub(crate) fn add_model(&mut self, model: Arc<AliasModel>) -> Result<u32, RenderError> {
        let poses = self.device_buffer(&pose_bytes(&model), BufferUsage::VERTEX_BUFFER)?;
        let uvs = self.device_buffer(&uv_bytes(&model), BufferUsage::VERTEX_BUFFER)?;
        let size = (model.skin_width, model.skin_height);
        let pictures: Vec<_> = model
            .skins
            .iter()
            .map(|skin| (size.0, size.1, skin.as_slice()))
            .collect();
        let (pool, skin_sets, skins) = self.upload_textures(&pictures)?;
        self.models.push(GpuAlias {
            model,
            poses,
            uvs,
            skin_sets,
            _skins: skins,
            _pool: pool,
        });
        Ok(u32::try_from(self.models.len() - 1).unwrap_or(u32::MAX))
    }

    /// Uploads a sprite, and returns its number.
    pub(crate) fn add_sprite(&mut self, sprite: Arc<Sprite>) -> Result<u32, RenderError> {
        let pictures: Vec<_> = sprite
            .images
            .iter()
            .map(|picture| (picture.width, picture.height, picture.rgba.as_slice()))
            .collect();
        let (pool, image_sets, images) = self.upload_textures(&pictures)?;
        self.sprites.push(GpuSprite {
            sprite,
            sets: image_sets,
            _images: images,
            _pool: pool,
        });
        Ok(u32::try_from(self.sprites.len() - 1).unwrap_or(u32::MAX))
    }

    /// Uploads `map`.
    pub(crate) fn load_map(&mut self, map: &Map, mesh: &WorldMesh) -> Result<(), RenderError> {
        self.wait_for_frame()?;
        self.device.wait_idle()?;
        self.map = None;
        let mut vertex_bytes = Vec::with_capacity(mesh.vertices.len() * Vertex::SIZE);
        for vertex in &mesh.vertices {
            vertex.write(&mut vertex_bytes);
        }
        let vertices = self.device_buffer(&vertex_bytes, BufferUsage::VERTEX_BUFFER)?;
        let world_indices = mesh.world_index_count as usize;
        let mut indices = host_buffer(
            &self.device,
            &self.physical,
            (world_indices + mesh.model_indices.len()) * 4,
            BufferUsage::INDEX_BUFFER,
        )?;
        {
            let mut mapped = indices.memory.map()?;
            let (target, _remainder) = mapped.as_slice_mut().as_chunks_mut::<4>();
            for (index, chunk) in mesh.model_indices.iter().zip(&mut target[world_indices..]) {
                *chunk = index.to_le_bytes();
            }
        }

        let sets = u32::try_from(map.textures.len() + 2).unwrap_or(u32::MAX);
        let material_count = map.textures.iter().filter(|t| t.material.is_some()).count();
        let images = sets.saturating_mul(8);
        let pool = DescriptorPool::new(
            &self.device,
            sets,
            &[
                // The frame set takes a few of each on top of one a set.
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::UNIFORM_BUFFER,
                    descriptor_count: sets.saturating_add(2),
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: images,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLER,
                    descriptor_count: sets.saturating_add(2),
                },
            ],
        )?;
        let atlas = self.upload_image(
            &[(mesh.atlas_size, mesh.atlas_size, mesh.atlas.as_slice())],
            Format::R8G8B8A8_UNORM,
        )?;
        let direction_atlas = self.upload_image(
            &[(
                mesh.atlas_size,
                mesh.atlas_size,
                mesh.direction_atlas.as_slice(),
            )],
            Format::R8G8B8A8_UNORM,
        )?;
        let paint_atlas = self.upload_image(
            &[(
                mesh.atlas_size,
                mesh.atlas_size,
                mesh.paint_atlas.as_slice(),
            )],
            Format::R8G8B8A8_UNORM,
        )?;
        // Every opaque face of the world, then every moving model, for shadows.
        let opaque_batch = |batch: &Option<u32>| {
            batch.is_some_and(|batch| {
                let kind = mesh.batches[batch as usize].kind;
                !matches!(
                    kind,
                    BatchKind::Sky | BatchKind::Blended | BatchKind::Liquid
                )
            })
        };
        let groups = shadow_groups(map, mesh, opaque_batch);
        let mut shadow_world = Vec::new();
        let mut shadow_parts = Vec::with_capacity(groups.len());
        for group in groups {
            shadow_parts.push(ShadowRange {
                first: u32::try_from(shadow_world.len()).unwrap_or(0),
                count: u32::try_from(group.indices.len()).unwrap_or(0),
                offset: Vec3::ZERO,
                bounds: group.bounds,
            });
            shadow_world.extend(group.indices);
        }
        let shadow_world_count = u32::try_from(shadow_world.len()).unwrap_or(0);
        let mut shadow_indices = host_buffer(
            &self.device,
            &self.physical,
            ((shadow_world.len() + mesh.model_indices.len()).max(1)) * 4,
            BufferUsage::INDEX_BUFFER,
        )?;
        {
            let mut mapped = shadow_indices.memory.map()?;
            let all = shadow_world.iter().chain(&mesh.model_indices);
            for (index, chunk) in all.zip(mapped.as_slice_mut().as_chunks_mut::<4>().0) {
                *chunk = index.to_le_bytes();
            }
        }
        self.shadows.forget();
        let frame_set = pool.allocate(&self.frame_layout)?;
        frame_set.write_buffer(
            0,
            DescriptorType::UNIFORM_BUFFER,
            &self.uniforms.buffer,
            0,
            FRAME_SIZE as u64,
        );
        frame_set.write_sampled_image(1, &atlas.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        frame_set.write_sampler(2, &self.light_sampler);
        frame_set.write_buffer(
            4,
            DescriptorType::UNIFORM_BUFFER,
            &self.live_uniforms.buffer,
            0,
            LIVE_BYTES as u64,
        );
        frame_set.write_sampled_image(
            5,
            &self.shadows.atlas.view,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );
        frame_set.write_sampler(6, &self.shadow_sampler);
        frame_set.write_sampled_image(7, &paint_atlas.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
        frame_set.write_sampled_image(
            3,
            &direction_atlas.view,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        );

        let blank =
            self.upload_image(&[(1, 1, &[128, 128, 255, 255][..])], Format::R8G8B8A8_UNORM)?;
        let mut material_params = (material_count > 0)
            .then(|| {
                host_buffer(
                    &self.device,
                    &self.physical,
                    material_count * MATERIAL_PARAMS_STRIDE,
                    BufferUsage::UNIFORM_BUFFER,
                )
            })
            .transpose()?;
        let mut next_params = 0;
        let mut textures = Vec::with_capacity(map.textures.len());
        let mut material_sets = Vec::with_capacity(map.textures.len());
        let mut texture_sets = Vec::with_capacity(map.textures.len());
        for texture in &map.textures {
            if texture.kind == TextureKind::Hidden {
                material_sets.push(false);
                texture_sets.push(None);
                continue;
            }
            let levels: Vec<(u32, u32, &[u8])> = texture
                .mips
                .iter()
                .map(|mip| (mip.width, mip.height, mip.rgba.as_slice()))
                .collect();
            let image = self.upload_image(&levels, Format::R8G8B8A8_UNORM)?;
            if let (Some(material), Some(params)) = (&texture.material, &mut material_params)
                && uses_material(texture)
            {
                let set = pool.allocate(&self.material_layout)?;
                set.write_sampled_image(0, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                set.write_sampler(1, &self.material_sampler);
                let mut uploaded = Vec::with_capacity(material.slots.len());
                for slot in &material.slots {
                    let levels: Vec<(u32, u32, &[u8])> = slot
                        .iter()
                        .map(|mip| (mip.width, mip.height, mip.rgba.as_slice()))
                        .collect();
                    uploaded.push(self.upload_image(&levels, Format::R8G8B8A8_UNORM)?);
                }
                for binding in 0..7_u32 {
                    let view = uploaded
                        .get(binding as usize)
                        .map_or(&blank.view, |image| &image.view);
                    set.write_sampled_image(
                        binding + 2,
                        view,
                        ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    );
                }
                let offset = next_params * MATERIAL_PARAMS_STRIDE;
                let bytes = material_params_of(material);
                params.memory.map()?.as_slice_mut()[offset..offset + bytes.len()]
                    .copy_from_slice(&bytes);
                set.write_buffer(
                    9,
                    DescriptorType::UNIFORM_BUFFER,
                    &params.buffer,
                    offset as u64,
                    MATERIAL_PARAMS_BYTES as u64,
                );
                next_params += 1;
                material_sets.push(true);
                texture_sets.push(Some(set));
                textures.push(image);
                textures.extend(uploaded);
                continue;
            }
            let set = pool.allocate(&self.texture_layout)?;
            set.write_sampled_image(0, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            set.write_sampler(1, &self.texture_sampler);
            material_sets.push(false);
            texture_sets.push(Some(set));
            textures.push(image);
        }
        let sky_set = match &map.sky {
            Some(sky) => {
                let cloud = sky.clouds.as_ref().and_then(|clouds| clouds.image.as_ref());
                let view = if let Some(cloud) = cloud {
                    let levels: Vec<(u32, u32, &[u8])> = cloud
                        .iter()
                        .map(|mip| (mip.width, mip.height, mip.rgba.as_slice()))
                        .collect();
                    textures.push(self.upload_image(&levels, Format::R8G8B8A8_UNORM)?);
                    textures.last().map(|image| &image.view)
                } else {
                    Some(&blank.view)
                };
                let set = pool.allocate(&self.texture_layout)?;
                if let Some(view) = view {
                    set.write_sampled_image(0, view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
                }
                set.write_sampler(1, &self.material_sampler);
                Some(set)
            }
            None => None,
        };
        self.load_stencils(map)?;
        self.map = Some(GpuMap {
            sky_set,
            material_sets,
            vertices,
            indices,
            shadow_indices,
            shadow_world_count,
            shadow_parts,
            texture_sets,
            _material_params: material_params,
            _textures: {
                let mut all = textures;
                all.push(blank);
                all
            },
            _pool: pool,
            frame_set,
            atlas,
            direction_atlas,
            paint_atlas,
        });
        Ok(())
    }

    /// A device-local buffer holding `bytes`.
    pub(super) fn device_buffer(
        &self,
        bytes: &[u8],
        usage: BufferUsage,
    ) -> Result<GpuBuffer, RenderError> {
        let mut staging = host_buffer(
            &self.device,
            &self.physical,
            bytes.len(),
            BufferUsage::TRANSFER_SRC,
        )?;
        staging.memory.map()?.as_slice_mut()[..bytes.len()].copy_from_slice(bytes);
        let (buffer, memory) = Buffer::new_bound(
            &self.device,
            &self.physical,
            BufferCreateInfo {
                size: bytes.len().max(4) as u64,
                usage: usage | BufferUsage::TRANSFER_DST,
            },
            MemoryPropertyFlags::DEVICE_LOCAL,
        )?;
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
                rec.copy_buffer(
                    &staging.buffer,
                    &buffer,
                    &[BufferCopy {
                        src_offset: 0,
                        dst_offset: 0,
                        size: bytes.len().max(4) as u64,
                    }],
                );
                Ok(())
            })?;
        Ok(GpuBuffer { buffer, memory })
    }

    /// An image holding `levels`, largest first, ready to sample.
    /// Points vertices at where their light is now.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub(crate) fn write_vertices(&self, patches: &[VertexPatch]) -> Result<(), RenderError> {
        self.write_tile(None, patches)
    }

    /// Copies a tile's light into the atlas at `corner`, and points vertices at
    /// it.
    ///
    /// # Errors
    ///
    /// Returns [`RenderError`] if the device fails.
    pub(crate) fn write_tile_light(
        &self,
        image: &TileImage,
        corner: [u32; 2],
        patches: &[VertexPatch],
    ) -> Result<(), RenderError> {
        self.write_tile(Some((image, corner)), patches)
    }

    /// Writes what's given in one go. Barriers wait for any frame still using
    /// what's written.
    fn write_tile(
        &self,
        image: Option<(&TileImage, [u32; 2])>,
        patches: &[VertexPatch],
    ) -> Result<(), RenderError> {
        let Some(map) = &self.map else {
            return Ok(());
        };
        let parts = image.iter().flat_map(|(image, _)| {
            [
                image.bytes.as_slice(),
                image.directions.as_slice(),
                image.paint.as_slice(),
            ]
        });
        let mut bytes = Vec::new();
        let mut offsets = Vec::new();
        for part in parts {
            offsets.push(bytes.len() as u64);
            bytes.extend_from_slice(part);
        }
        let mut copies = Vec::with_capacity(patches.len());
        for (first, patch) in patches {
            copies.push(BufferCopy {
                src_offset: bytes.len() as u64,
                dst_offset: u64::from(*first) * Vertex::SIZE as u64,
                size: patch.len() as u64,
            });
            bytes.extend_from_slice(patch);
        }
        if bytes.is_empty() {
            return Ok(());
        }
        let mut staging = host_buffer(
            &self.device,
            &self.physical,
            bytes.len(),
            BufferUsage::TRANSFER_SRC,
        )?;
        staging.memory.map()?.as_slice_mut()[..bytes.len()].copy_from_slice(&bytes);
        let atlases = [&map.atlas, &map.direction_atlas, &map.paint_atlas];
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
                if let Some((image, corner)) = image {
                    for (atlas, offset) in atlases.iter().zip(&offsets) {
                        copy_into_atlas(rec, &staging, atlas, *offset, image.size, corner);
                    }
                }
                if !copies.is_empty() {
                    rec.memory_barrier(
                        PipelineStage::ALL_GRAPHICS,
                        PipelineStage::TRANSFER,
                        AccessFlags::VERTEX_ATTRIBUTE_READ,
                        AccessFlags::TRANSFER_WRITE,
                    );
                    rec.copy_buffer(&staging.buffer, &map.vertices.buffer, &copies);
                    rec.memory_barrier(
                        PipelineStage::TRANSFER,
                        PipelineStage::ALL_GRAPHICS,
                        AccessFlags::TRANSFER_WRITE,
                        AccessFlags::VERTEX_ATTRIBUTE_READ,
                    );
                }
                Ok(())
            })?;
        Ok(())
    }

    pub(super) fn upload_image(
        &self,
        levels: &[(u32, u32, &[u8])],
        format: Format,
    ) -> Result<GpuImage, RenderError> {
        let &(width, height, _) = levels
            .first()
            .ok_or_else(|| RenderError("an image with no pixels".to_owned()))?;
        let image = Image::new_2d_array(
            &self.device,
            Image2dCreateInfo {
                format,
                width,
                height,
                usage: ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
            },
            1,
            u32::try_from(levels.len()).unwrap_or(1),
        )?;
        let image = device_image(&self.device, &self.physical, image, false)?;
        let mut pixels = Vec::new();
        let mut regions = Vec::with_capacity(levels.len());
        for (level, &(width, height, bytes)) in levels.iter().enumerate() {
            let mut copy = BufferImageCopy::full_2d(width, height);
            copy.buffer_offset = pixels.len() as u64;
            copy.mip_level = u32::try_from(level).unwrap_or(0);
            regions.push(copy);
            pixels.extend_from_slice(bytes);
        }
        let mut staging = host_buffer(
            &self.device,
            &self.physical,
            pixels.len(),
            BufferUsage::TRANSFER_SRC,
        )?;
        staging.memory.map()?.as_slice_mut()[..pixels.len()].copy_from_slice(&pixels);
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
                    &staging.buffer,
                    &image.image,
                    ImageLayout::TRANSFER_DST_OPTIMAL,
                    &regions,
                );
                rec.image_barrier(
                    PipelineStage::TRANSFER,
                    PipelineStage::FRAGMENT_SHADER,
                    ImageBarrier::color(
                        &image.image,
                        ImageLayout::TRANSFER_DST_OPTIMAL,
                        ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        AccessFlags::TRANSFER_WRITE,
                        AccessFlags::SHADER_READ,
                    ),
                );
                Ok(())
            })?;
        Ok(image)
    }

    /// Writes the uniforms, and the indices if they changed.
    pub(super) fn upload_frame(
        &mut self,
        view: &View,
        scene: Option<&Scene<'_>>,
        aspect: f32,
    ) -> Result<(), RenderError> {
        let lights = scene.map_or(&[][..], |scene| scene.drawables.lights);
        let sky = scene.and_then(|scene| scene.map.sky.as_ref());
        let fog = scene.and_then(|scene| scene.map.fog.as_ref());
        let extra = FrameExtras {
            stencils: scene.map_or(&[][..], |scene| scene.map.stencils.as_slice()),
            lights: scene.map_or(&[][..], |scene| scene.drawables.stencil_lights),
            masks: scene.map_or(&[][..], |scene| scene.drawables.stencil_masks),
            capsules: scene.map_or(&[][..], |scene| scene.drawables.capsules),
            pools: scene.map_or(&[][..], |scene| scene.map.pools.as_slice()),
            eye: view.origin,
            under_sky: scene.is_some_and(|scene| scene.map.open_above(view.origin)),
            samples: self.samples,
            probes: scene.and_then(|scene| scene.map.probes.as_ref()),
        };
        let bytes = frame_bytes(view, aspect, lights, sky, fog, &self.post, &extra);
        self.uniforms.memory.map()?.as_slice_mut()[..bytes.len()].copy_from_slice(&bytes);
        let live = scene.map_or(&[][..], |scene| scene.drawables.live_lights);
        let slots = match scene.filter(|_| self.post.live_shadows) {
            Some(scene) => {
                let figures = shadow_figures(&self.models, scene.drawables.entities);
                let instances = scene.drawables.instances;
                self.shadows
                    .plan(live, instances, &figures, scene.map, view.origin)
            }
            None => {
                self.shadows.rest();
                vec![None; live.len()]
            }
        };
        let size = self.scene_size();
        let bytes = live_bytes(view, aspect, size, live, &slots);
        self.live_uniforms.memory.map()?.as_slice_mut()[..bytes.len()].copy_from_slice(&bytes);
        if let Some(scene) = scene
            && !scene.drawables.particles.is_empty()
        {
            let mut mapped = self.particles.memory.map()?;
            let (target, _remainder) = mapped.as_slice_mut().as_chunks_mut::<PARTICLE_BYTES>();
            for (particle, chunk) in scene.drawables.particles.iter().zip(target) {
                write_particle(particle, chunk);
            }
        }
        if let (Some(scene), Some(map)) = (scene, &mut self.map)
            && scene.changed
        {
            let mut mapped = map.indices.memory.map()?;
            let (target, _remainder) = mapped.as_slice_mut().as_chunks_mut::<4>();
            for (index, chunk) in scene.visible.indices.iter().zip(target) {
                *chunk = index.to_le_bytes();
            }
        }
        Ok(())
    }
}
