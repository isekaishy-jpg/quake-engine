//! Land on the GPU (proposal 0144).

use engine_core::math::Vec3;
use engine_map::{Map, MipLevel, PixelForm, SlotImage, TerrainSheet};
use vulkane::safe::{
    AccessFlags, BufferCopy, BufferImageCopy, BufferUsage, DescriptorPool, DescriptorPoolSize,
    DescriptorSet, DescriptorType, Format, Image, Image2dCreateInfo, ImageBarrier, ImageLayout,
    ImageUsage, PipelineStage,
};

use super::upload::{compiled_format, device_image, host_buffer};
use super::{Backend, FRAMES, GpuBuffer, GpuImage};
use crate::error::RenderError;
use crate::terrain::{
    ChunkLight, GRIDS, NODE_BYTES, TerrainBatch, chunk_list, chunk_ranges, grid_indices,
    height_bytes, hole_bytes, layer_bytes, normal_bytes, paint_bytes, sheet_bytes, sheet_normals,
    sheet_ranges,
};

/// Parts of the land in a frame. Past this, what's furthest goes undrawn.
pub(super) const MOST_NODES: usize = 1 << 13;

/// The land of a map, on the GPU.
pub(super) struct GpuTerrain {
    /// Each sheet, kept alive for the set.
    _sheets: GpuBuffer,
    /// Every height.
    heights: GpuBuffer,
    /// Which way they face.
    normals: GpuBuffer,
    /// Its holes.
    holes: GpuBuffer,
    /// What it's painted with.
    paint: GpuBuffer,
    /// Each layer, kept alive for the set.
    _layers: GpuBuffer,
    /// The colour of what it's painted with, kept alive for the set.
    _colors: GpuImage,
    /// The bumps, height and shine of what it's painted with, kept alive for the set.
    _surfaces: GpuImage,
    /// Where each sheet's heights, holes and paint start, in samples and in bytes.
    firsts: Vec<(u64, u64, u64)>,
    /// How many chunks come before each sheet.
    first_chunks: Vec<u32>,
    /// What reads those.
    pub(super) set: DescriptorSet,
    /// Kept alive for the set.
    _pool: DescriptorPool,
    /// The grids' triangles.
    pub(super) indices: GpuBuffer,
    /// Where each grid's start, and how many there are.
    pub(super) grids: Vec<(u32, u32)>,
    /// What's drawn this frame, for each frame in flight.
    pub(super) nodes: Vec<GpuBuffer>,
    /// How that's grouped.
    pub(super) batches: [Vec<TerrainBatch>; FRAMES],
    /// Where each chunk's light is.
    pub(super) lights: Vec<ChunkLight>,
    /// The low and high height of each chunk.
    pub(super) ranges: Vec<[f32; 2]>,
    /// Every chunk, at the level shadows use, as a part of the land to draw.
    pub(super) whole: GpuBuffer,
    /// Each chunk's middle and how far it reaches, in the same order.
    pub(super) whole_bounds: Vec<(Vec3, f32)>,
}

impl Backend {
    /// Writes `bytes` over part of `target`.
    fn write_into(&self, target: &GpuBuffer, at: u64, bytes: &[u8]) -> Result<(), RenderError> {
        if bytes.is_empty() {
            return Ok(());
        }
        let mut staging = host_buffer(
            &self.device,
            &self.physical,
            bytes.len(),
            BufferUsage::TRANSFER_SRC,
        )?;
        staging.memory.map()?.as_slice_mut()[..bytes.len()].copy_from_slice(bytes);
        self.queue
            .one_shot(&self.device, self.queue_family, |rec| {
                rec.copy_buffer(
                    &staging.buffer,
                    &target.buffer,
                    &[BufferCopy {
                        src_offset: 0,
                        dst_offset: at,
                        size: bytes.len() as u64,
                    }],
                );
                Ok(())
            })?;
        Ok(())
    }

    /// Puts sheet `number` back on the GPU with what's changed in it, for a brush, which can't wait
    /// for a whole map.
    pub(crate) fn update_land(
        &mut self,
        number: usize,
        sheet: &TerrainSheet,
    ) -> Result<(), RenderError> {
        self.wait_for_frame()?;
        self.device.wait_idle()?;
        let first_chunk: u32 = self.loaded_sheet_chunks(number);
        if let Some(land) = self.map.as_mut().and_then(|map| map.terrain.as_mut()) {
            let ranges = sheet_ranges(sheet);
            let first = first_chunk as usize;
            if let Some(slots) = land.ranges.get_mut(first..first + ranges.len()) {
                slots.copy_from_slice(&ranges);
            }
        }
        let Some(land) = self.map.as_ref().and_then(|map| map.terrain.as_ref()) else {
            return Ok(());
        };
        let Some(&(samples, holes, paint)) = land.firsts.get(number) else {
            return Ok(());
        };
        let heights: Vec<u8> = sheet
            .data
            .heights
            .iter()
            .flat_map(|height| height.to_le_bytes())
            .collect();
        self.write_into(&land.heights, samples * 4, &heights)?;
        self.write_into(&land.normals, samples * 4, &sheet_normals(sheet))?;
        let mut hole_data = sheet.data.holes.clone();
        hole_data.resize(hole_data.len().next_multiple_of(4), 0);
        self.write_into(&land.holes, holes, &hole_data)?;
        // Paint that wasn't there when the map loaded comes with the next build of it.
        if !sheet.pictures.is_empty() {
            self.write_into(&land.paint, paint, sheet.data.paint.as_flattened())?;
        }
        Ok(())
    }

    /// How many chunks come before sheet `number`.
    fn loaded_sheet_chunks(&self, number: usize) -> u32 {
        let land = self.map.as_ref().and_then(|map| map.terrain.as_ref());
        land.and_then(|land| land.first_chunks.get(number).copied())
            .unwrap_or(0)
    }

    /// `pictures` as one array, which has at least two layers, as one layer wouldn't be seen as
    /// an array.
    pub(super) fn upload_array(
        &self,
        pictures: &[&SlotImage],
        format: Format,
    ) -> Result<GpuImage, RenderError> {
        let plain = std::sync::Arc::new(vec![MipLevel {
            width: 1,
            height: 1,
            rgba: vec![255; 4],
            form: PixelForm::Rgba,
        }]);
        let mut layers: Vec<&SlotImage> = pictures.to_vec();
        while layers.len() < 2 {
            layers.push(layers.first().copied().unwrap_or(&plain));
        }
        let first = layers.first().map_or(&[][..], |layer| &layer[..]);
        let top = first
            .first()
            .ok_or_else(|| RenderError("a picture for land with no pixels".to_owned()))?;
        let image = Image::new_2d_array(
            &self.device,
            Image2dCreateInfo {
                format,
                width: top.width,
                height: top.height,
                usage: ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
            },
            u32::try_from(layers.len()).unwrap_or(1),
            u32::try_from(first.len()).unwrap_or(1),
        )?;
        let image = device_image(&self.device, &self.physical, image, false)?;
        let mut pixels = Vec::new();
        let mut regions = Vec::new();
        for (number, layer) in layers.iter().enumerate() {
            for (level, mip) in layer.iter().enumerate() {
                let mut copy = BufferImageCopy::full_2d(mip.width, mip.height);
                copy.buffer_offset = pixels.len() as u64;
                copy.mip_level = u32::try_from(level).unwrap_or(0);
                copy.base_array_layer = u32::try_from(number).unwrap_or(0);
                regions.push(copy);
                pixels.extend_from_slice(&mip.rgba);
            }
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

    /// What `map`'s land is painted with: its colours and its surfaces.
    fn upload_land_pictures(&self, map: &Map) -> Result<(GpuImage, GpuImage), RenderError> {
        let colors: Vec<&SlotImage> = map.land_pictures.iter().map(|p| &p.color).collect();
        let surfaces: Vec<&SlotImage> = map.land_pictures.iter().map(|p| &p.surface).collect();
        let format_of = |list: &[&SlotImage], plain: Format| {
            list.first()
                .and_then(|mips| compiled_format(mips))
                .unwrap_or(plain)
        };
        Ok((
            self.upload_array(&colors, format_of(&colors, Format::R8G8B8A8_SRGB))?,
            self.upload_array(&surfaces, format_of(&surfaces, Format::R8G8B8A8_UNORM))?,
        ))
    }

    /// Moves the light of these chunks of land, as tiles come and go.
    pub(crate) fn set_chunk_lights(&mut self, lights: &[(u32, ChunkLight)]) {
        let land = self.map.as_mut().and_then(|map| map.terrain.as_mut());
        if let Some(land) = land {
            for (chunk, light) in lights {
                if let Some(slot) = land.lights.get_mut(*chunk as usize) {
                    *slot = *light;
                }
            }
        }
    }

    /// Puts `map`'s land on the GPU, if it has any.
    pub(super) fn upload_terrain(
        &self,
        map: &Map,
        lights: Vec<ChunkLight>,
    ) -> Result<Option<GpuTerrain>, RenderError> {
        if map.terrain.is_empty() {
            return Ok(None);
        }
        let storage = BufferUsage::STORAGE_BUFFER;
        let sheet_data = sheet_bytes(map);
        let height_data = height_bytes(map);
        let normal_data = normal_bytes(map);
        let sheets = self.device_buffer(&sheet_data, storage)?;
        let heights = self.device_buffer(&height_data, storage)?;
        let normals = self.device_buffer(&normal_data, storage)?;
        let hole_data = hole_bytes(map);
        let holes = self.device_buffer(&hole_data, storage)?;
        // A buffer can't be bound with nothing in it.
        let mut paint_data = paint_bytes(map);
        paint_data.resize(paint_data.len().max(4), 0);
        let paint = self.device_buffer(&paint_data, storage)?;
        let mut layer_data = layer_bytes(map);
        layer_data.resize(layer_data.len().max(16), 0);
        let layers = self.device_buffer(&layer_data, storage)?;
        let (colors, surfaces) = self.upload_land_pictures(map)?;
        let pool = DescriptorPool::new(
            &self.device,
            1,
            &[
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::STORAGE_BUFFER,
                    descriptor_count: 6,
                },
                DescriptorPoolSize {
                    descriptor_type: DescriptorType::SAMPLED_IMAGE,
                    descriptor_count: 2,
                },
            ],
        )?;
        let set = pool.allocate(&self.terrain_set_layout)?;
        let kind = DescriptorType::STORAGE_BUFFER;
        set.write_buffer(0, kind, &sheets.buffer, 0, sheet_data.len() as u64);
        set.write_buffer(1, kind, &heights.buffer, 0, height_data.len() as u64);
        set.write_buffer(2, kind, &normals.buffer, 0, normal_data.len() as u64);
        set.write_buffer(3, kind, &holes.buffer, 0, hole_data.len() as u64);
        set.write_buffer(4, kind, &paint.buffer, 0, paint_data.len() as u64);
        set.write_buffer(5, kind, &layers.buffer, 0, layer_data.len() as u64);
        let shown = ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        set.write_sampled_image(6, &colors.view, shown);
        set.write_sampled_image(7, &surfaces.view, shown);
        let mut index_bytes = Vec::new();
        let mut grids = Vec::with_capacity(GRIDS.len());
        for size in GRIDS {
            let list = grid_indices(size);
            let first = u32::try_from(index_bytes.len() / 4).unwrap_or(0);
            grids.push((first, u32::try_from(list.len()).unwrap_or(0)));
            index_bytes.extend(list.iter().flat_map(|index| index.to_le_bytes()));
        }
        let indices = self.device_buffer(&index_bytes, BufferUsage::INDEX_BUFFER)?;
        let mut nodes = Vec::with_capacity(FRAMES);
        for _ in 0..FRAMES {
            nodes.push(host_buffer(
                &self.device,
                &self.physical,
                MOST_NODES * NODE_BYTES,
                BufferUsage::VERTEX_BUFFER,
            )?);
        }
        let mut firsts = Vec::with_capacity(map.terrain.len());
        let (mut samples, mut hole_at, mut paint_at) = (0_u64, 0_u64, 0_u64);
        for sheet in &map.terrain {
            firsts.push((samples, hole_at, paint_at));
            samples += sheet.data.heights.len() as u64;
            hole_at += sheet.data.holes.len().next_multiple_of(4) as u64;
            if !sheet.pictures.is_empty() {
                paint_at += (sheet.data.paint.len() * 8) as u64;
            }
        }
        let (whole_bytes, whole_bounds) = chunk_list(map);
        let whole = self.device_buffer(&whole_bytes, BufferUsage::VERTEX_BUFFER)?;
        Ok(Some(GpuTerrain {
            _sheets: sheets,
            heights,
            normals,
            holes,
            paint,
            _layers: layers,
            _colors: colors,
            _surfaces: surfaces,
            firsts,
            first_chunks: map
                .terrain
                .iter()
                .scan(0, |before, sheet| {
                    let first = *before;
                    *before += sheet.chunk_count();
                    Some(first)
                })
                .collect(),
            ranges: chunk_ranges(map),
            set,
            _pool: pool,
            indices,
            grids,
            nodes,
            batches: std::array::from_fn(|_| Vec::new()),
            lights,
            whole,
            whole_bounds,
        }))
    }
}
