//! Getting what's drawn onto the GPU.

use std::sync::Arc;

use engine_core::canvas::Quad;
use engine_map::{AliasModel, Map, Sprite, Texture, TextureKind};
use vulkane::safe::{
    AccessFlags, Buffer, BufferCopy, BufferCreateInfo, BufferImageCopy, BufferUsage,
    DescriptorPool, DescriptorPoolSize, DescriptorSet, DescriptorType, Device, DeviceMemory,
    Format, Image, Image2dCreateInfo, ImageBarrier, ImageLayout, ImageUsage, ImageView,
    MemoryPropertyFlags, PhysicalDevice, PipelineStage,
};

use super::{
    Backend, DEPTH_FORMAT, FRAME_SIZE, GpuAlias, GpuBuffer, GpuImage, GpuMap, GpuPicture,
    GpuSprite, Scene,
};
use crate::alias::{pose_bytes, uv_bytes};
use crate::canvas::canvas_vertices;
use crate::error::RenderError;
use crate::material::{
    MATERIAL_PARAMS_BYTES, MATERIAL_PARAMS_STRIDE, material_params as material_params_of,
};
use crate::matrix::view_projection;
use crate::renderer::View;
use crate::sprite::{LightDraw, MAX_LIGHT_DRAWS, PARTICLE_BYTES, write_particle};
use crate::world::{Vertex, WorldMesh};

/// A buffer the CPU can write.
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

/// Whether `texture` is drawn through the material pipeline.
fn batch_uses_material(texture: &Texture) -> bool {
    matches!(texture.kind, TextureKind::Solid | TextureKind::Cutout)
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
pub(super) fn frame_bytes(view: &View, aspect: f32, lights: &[LightDraw]) -> Vec<u8> {
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
    let mut out = Vec::with_capacity(FRAME_SIZE);
    for value in matrix
        .iter()
        .chain(&eye)
        .chain(&params)
        .chain(&view.styles)
        .chain(&positions)
        .chain(&cuts)
    {
        out.extend_from_slice(&value.to_le_bytes());
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
        for (value, chunk) in floats.iter().zip(target) {
            *chunk = value.to_le_bytes();
        }
        drop(mapped);
        canvas.batches = batches;
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

        let sets = u32::try_from(map.textures.len() + 1).unwrap_or(u32::MAX);
        let material_count = map.textures.iter().filter(|t| t.material.is_some()).count();
        let images = sets.saturating_mul(8);
        let pool = DescriptorPool::new(
            &self.device,
            sets,
            &[
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::UNIFORM_BUFFER,
                    descriptor_count: sets,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: images,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLER,
                    descriptor_count: sets,
                },
            ],
        )?;
        let atlas = self.upload_image(
            &[(mesh.atlas_size, mesh.atlas_size, mesh.atlas.as_slice())],
            Format::R8G8B8A8_UNORM,
        )?;
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
        let mut texture_sets = Vec::with_capacity(map.textures.len());
        for texture in &map.textures {
            if texture.kind == TextureKind::Hidden {
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
                && batch_uses_material(texture)
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
                texture_sets.push(Some(set));
                textures.push(image);
                textures.extend(uploaded);
                continue;
            }
            let set = pool.allocate(&self.texture_layout)?;
            set.write_sampled_image(0, &image.view, ImageLayout::SHADER_READ_ONLY_OPTIMAL);
            set.write_sampler(1, &self.texture_sampler);
            texture_sets.push(Some(set));
            textures.push(image);
        }
        self.map = Some(GpuMap {
            vertices,
            indices,
            texture_sets,
            _material_params: material_params,
            _textures: {
                let mut all = textures;
                all.push(blank);
                all
            },
            _pool: pool,
            frame_set,
            _atlas: atlas,
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
        let bytes = frame_bytes(view, aspect, lights);
        self.uniforms.memory.map()?.as_slice_mut()[..bytes.len()].copy_from_slice(&bytes);
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
