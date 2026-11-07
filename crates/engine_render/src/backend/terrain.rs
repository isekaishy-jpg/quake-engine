//! Land on the GPU (proposal 0144).

use engine_map::Map;
use vulkane::safe::{
    BufferUsage, DescriptorPool, DescriptorPoolSize, DescriptorSet, DescriptorType,
};

use super::upload::host_buffer;
use super::{Backend, FRAMES, GpuBuffer};
use crate::error::RenderError;
use crate::terrain::{
    ChunkLight, GRIDS, NODE_BYTES, TerrainBatch, grid_indices, height_bytes, normal_bytes,
    sheet_bytes,
};

/// Parts of the land in a frame. Past this, what's furthest goes undrawn.
pub(super) const MOST_NODES: usize = 1 << 13;

/// The land of a map, on the GPU.
pub(super) struct GpuTerrain {
    /// Each sheet, kept alive for the set.
    _sheets: GpuBuffer,
    /// Every height, kept alive for the set.
    _heights: GpuBuffer,
    /// Which way they face, kept alive for the set.
    _normals: GpuBuffer,
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
}

impl Backend {
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
        let pool = DescriptorPool::new(
            &self.device,
            1,
            &[DescriptorPoolSize {
                descriptor_type: DescriptorType::STORAGE_BUFFER,
                descriptor_count: 3,
            }],
        )?;
        let set = pool.allocate(&self.terrain_set_layout)?;
        let kind = DescriptorType::STORAGE_BUFFER;
        set.write_buffer(0, kind, &sheets.buffer, 0, sheet_data.len() as u64);
        set.write_buffer(1, kind, &heights.buffer, 0, height_data.len() as u64);
        set.write_buffer(2, kind, &normals.buffer, 0, normal_data.len() as u64);
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
        Ok(Some(GpuTerrain {
            _sheets: sheets,
            _heights: heights,
            _normals: normals,
            set,
            _pool: pool,
            indices,
            grids,
            nodes,
            batches: std::array::from_fn(|_| Vec::new()),
            lights,
        }))
    }
}
