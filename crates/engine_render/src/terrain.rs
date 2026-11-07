//! Which parts of the land to draw, and how finely (proposal 0144). It's CDLOD: a tree over the
//! chunks picks a stride for each part by distance, and near the end of its range a vertex slides
//! onto the next coarser lattice, so there's nothing to stitch.

use engine_core::math::Vec3;
use engine_map::{CHUNK_CELLS, Map, TerrainSheet};

use crate::instances::ball_in_view;

/// Bytes in what's kept for each part of the land that's drawn.
pub(crate) const NODE_BYTES: usize = 64;

/// The sizes of grid that a part of the land is drawn with, in cells.
pub const GRIDS: [u32; 4] = [32, 16, 8, 4];

/// How far from the view each level reaches. Each is twice the last, which keeps neighbours within
/// a level of each other, as sliding needs.
pub const LEVEL_REACH: [f32; 4] = [1200.0, 2400.0, 4800.0, 9600.0];

/// How far through its reach a level begins to slide onto the next.
const SLIDE_FROM: f32 = 0.7;

/// How many cells apart a level's vertices are.
const fn stride_of(level: usize) -> u32 {
    1 << level
}

/// Where a chunk's light is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChunkLight {
    /// The atlas place of its low corner.
    pub low: [f32; 2],
    /// The atlas place of its high corner.
    pub high: [f32; 2],
    /// The distance across the atlas to the next style.
    pub step: f32,
    /// The style of each layer.
    pub styles: [u8; 4],
}

/// The parts of a sheet that are drawn on one size of grid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerrainBatch {
    /// Which sheet.
    pub sheet: u32,
    /// Which of [`GRIDS`].
    pub grid: u32,
    /// The first of them in the frame's list.
    pub first: u32,
    /// How many there are.
    pub count: u32,
}

/// One part of the land to draw.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TerrainNode {
    /// Which sheet.
    pub sheet: u32,
    /// The sample of its low corner.
    pub first: [u32; 2],
    /// How many cells apart its vertices are.
    pub stride: u32,
    /// How many cells of its grid there are along a side.
    pub grid: u32,
    /// Where its vertices begin to slide, and where they've arrived.
    pub slide: [f32; 2],
    /// Its chunk, among all of them.
    pub chunk: u32,
}

/// How far `eye` is from a box.
fn distance_to(eye: Vec3, low: Vec3, high: Vec3) -> f32 {
    let along = |eye: f32, low: f32, high: f32| (low - eye).max(eye - high).max(0.0);
    Vec3::new(
        along(eye.x, low.x, high.x),
        along(eye.y, low.y, high.y),
        along(eye.z, low.z, high.z),
    )
    .length()
}

/// The range of a part of a chunk: its low and high corners.
fn bounds(sheet: &TerrainSheet, first: [u32; 2], cells: u32, heights: [f32; 2]) -> (Vec3, Vec3) {
    let low = sheet.sample(first[0], first[1]);
    let high = sheet.sample(first[0] + cells, first[1] + cells);
    (
        Vec3::new(low.x, low.y, heights[0]),
        Vec3::new(high.x, high.y, heights[1]),
    )
}

/// The level for something `distance` away.
fn level_at(distance: f32) -> usize {
    LEVEL_REACH
        .iter()
        .position(|reach| distance < *reach)
        .unwrap_or(LEVEL_REACH.len())
}

/// A node at `level`, of `cells` cells from `first`.
fn node_at(level: usize, first: [u32; 2], cells: u32) -> TerrainNode {
    let stride = stride_of(level);
    let reach = LEVEL_REACH.get(level).copied();
    TerrainNode {
        first,
        stride,
        grid: cells / stride,
        // The coarsest has nothing to slide onto.
        slide: reach.map_or([f32::MAX, f32::MAX], |reach| [reach * SLIDE_FROM, reach]),
        ..TerrainNode::default()
    }
}

/// The parts of chunk `x`, `y` of `sheet` to draw from `eye`: all of it at one level, or its
/// quarters if it's near enough for the finest.
fn chunk_nodes(sheet: &TerrainSheet, (x, y): (u32, u32), eye: Vec3, out: &mut Vec<TerrainNode>) {
    let heights = sheet.chunk_heights(x, y);
    let first = [x * CHUNK_CELLS, y * CHUNK_CELLS];
    let (low, high) = bounds(sheet, first, CHUNK_CELLS, heights);
    let level = level_at(distance_to(eye, low, high));
    if level > 0 {
        out.push(node_at(level, first, CHUNK_CELLS));
        return;
    }
    let half = CHUNK_CELLS / 2;
    for corner in [[0, 0], [half, 0], [0, half], [half, half]] {
        let first = [first[0] + corner[0], first[1] + corner[1]];
        let (low, high) = bounds(sheet, first, half, heights);
        let level = level_at(distance_to(eye, low, high)).min(1);
        out.push(node_at(level, first, half));
    }
}

/// Writes the parts of the land that `planes` can see, among `chunks`, to `out`, grid by grid so
/// that each is one draw, and says where they went.
pub fn terrain_nodes(
    map: &Map,
    (chunks, lights): (&[u32], &[ChunkLight]),
    (eye, planes): (Vec3, &[[f32; 4]; 6]),
    out: &mut [u8],
) -> Vec<TerrainBatch> {
    let mut grouped: Vec<[Vec<TerrainNode>; GRIDS.len()]> = Vec::new();
    grouped.resize_with(map.terrain.len(), Default::default);
    let mut first_chunk = 0;
    let mut found = Vec::new();
    for (number, sheet) in (0_u32..).zip(&map.terrain) {
        let [across, _] = sheet.data.chunks();
        let last_chunk = first_chunk + sheet.chunk_count();
        for &chunk in chunks
            .iter()
            .filter(|c| (first_chunk..last_chunk).contains(*c))
        {
            let at = chunk - first_chunk;
            let (x, y) = (at % across, at / across);
            let heights = sheet.chunk_heights(x, y);
            let first = [x * CHUNK_CELLS, y * CHUNK_CELLS];
            let (low, high) = bounds(sheet, first, CHUNK_CELLS, heights);
            let middle = (low + high) * 0.5;
            if !ball_in_view(planes, middle, (high - low).length() * 0.5) {
                continue;
            }
            found.clear();
            chunk_nodes(sheet, (x, y), eye, &mut found);
            for node in &found {
                let Some(grid) = GRIDS.iter().position(|size| *size == node.grid) else {
                    continue;
                };
                grouped[number as usize][grid].push(TerrainNode {
                    sheet: number,
                    chunk,
                    ..*node
                });
            }
        }
        first_chunk = last_chunk;
    }
    let mut batches = Vec::new();
    let mut slots = out.as_chunks_mut::<NODE_BYTES>().0.iter_mut();
    let mut first = 0;
    for (sheet, grids) in (0_u32..).zip(&grouped) {
        for (grid, nodes) in (0_u32..).zip(grids) {
            let mut count = 0;
            for node in nodes {
                let Some(slot) = slots.next() else {
                    break;
                };
                let light = lights.get(node.chunk as usize).copied().unwrap_or_default();
                write_node(node, &light, slot);
                count += 1;
            }
            if count > 0 {
                batches.push(TerrainBatch {
                    sheet,
                    grid,
                    first,
                    count,
                });
                first += count;
            }
        }
    }
    batches
}

/// Lays `node` out as the shader reads it.
fn write_node(node: &TerrainNode, light: &ChunkLight, out: &mut [u8; NODE_BYTES]) {
    let words = [node.first[0], node.first[1], node.stride, node.grid];
    let floats = [
        node.slide[0],
        node.slide[1],
        light.low[0],
        light.low[1],
        light.high[0],
        light.high[1],
        light.step,
        0.0,
    ];
    let tail = [node.sheet, u32::from_le_bytes(light.styles), node.chunk, 0];
    let bytes = words
        .iter()
        .map(|word| word.to_le_bytes())
        .chain(floats.iter().map(|float| float.to_le_bytes()))
        .chain(tail.iter().map(|word| word.to_le_bytes()));
    for (chunk, value) in out.as_chunks_mut::<4>().0.iter_mut().zip(bytes) {
        *chunk = value;
    }
}

/// A grid of `size` cells each way, as indices, split as collision splits a cell.
#[must_use]
pub fn grid_indices(size: u32) -> Vec<u32> {
    let mut indices = Vec::with_capacity((size * size * 6) as usize);
    let at = |x: u32, y: u32| y * (size + 1) + x;
    for y in 0..size {
        for x in 0..size {
            let (a, b, c, d) = (at(x, y), at(x + 1, y), at(x + 1, y + 1), at(x, y + 1));
            indices.extend([a, b, c, a, c, d]);
        }
    }
    indices
}

/// What the shader's told of each sheet: its origin and spacing, where its samples are, and how a
/// place becomes a place on its texture.
pub(crate) fn sheet_bytes(map: &Map) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(map.terrain.len() * 48);
    let mut first_sample = 0_u32;
    for sheet in &map.terrain {
        let origin = [
            sheet.origin.x,
            sheet.origin.y,
            sheet.origin.z,
            sheet.data.cell,
        ];
        bytes.extend(origin.iter().flat_map(|part| part.to_le_bytes()));
        let shape = [
            sheet.data.across(),
            first_sample,
            u32::from(sheet.facing_down),
            0,
        ];
        bytes.extend(shape.iter().flat_map(|part| part.to_le_bytes()));
        let texture = map.textures.get(sheet.texture as usize);
        let size = texture.map_or([1.0, 1.0], texture_size);
        let uv = [1.0 / size[0], 1.0 / size[1], 0.0, 0.0];
        bytes.extend(uv.iter().flat_map(|part: &f32| part.to_le_bytes()));
        first_sample += u32::try_from(sheet.data.heights.len()).unwrap_or(0);
    }
    bytes
}

/// How many world units one repeat of `texture` covers.
#[expect(clippy::cast_precision_loss, reason = "a texture's size")]
fn texture_size(texture: &engine_map::Texture) -> [f32; 2] {
    texture.tile_size().unwrap_or_else(|| {
        texture.mips.first().map_or([64.0, 64.0], |mip| {
            [mip.width.max(1) as f32, mip.height.max(1) as f32]
        })
    })
}

/// Every sheet's heights, one after the other.
pub(crate) fn height_bytes(map: &Map) -> Vec<u8> {
    let heights = map.terrain.iter().flat_map(|sheet| &sheet.data.heights);
    heights.flat_map(|height| height.to_le_bytes()).collect()
}

/// Which way every sheet faces at each sample, as packed bytes.
pub(crate) fn normal_bytes(map: &Map) -> Vec<u8> {
    let mut bytes = Vec::new();
    for sheet in &map.terrain {
        for y in 0..=sheet.data.cells[1] {
            for x in 0..=sheet.data.cells[0] {
                let normal = sheet.normal(x, y);
                #[expect(clippy::cast_possible_truncation, reason = "kept to a byte")]
                let byte = |part: f32| (part * 127.0).round() as i8;
                let packed = [byte(normal.x), byte(normal.y), byte(normal.z), 0];
                bytes.extend(packed.map(i8::to_le_bytes).into_iter().flatten());
            }
        }
    }
    bytes
}
