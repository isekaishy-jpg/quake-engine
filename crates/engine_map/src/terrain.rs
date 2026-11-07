//! Land: sheets of heights on a grid (proposal 0144).

use engine_core::math::Vec3;

/// Cells along a side of a chunk, which is a tile of 2048 units at the usual spacing.
pub const CHUNK_CELLS: u32 = 64;

/// How far apart heights are unless a sheet says, which is a player's width.
pub const USUAL_CELL: f32 = 32.0;

/// The heights of some land, with nothing about where it goes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerrainData {
    /// How far apart its heights are.
    pub cell: f32,
    /// How many cells it has each way, which are a whole number of chunks.
    pub cells: [u32; 2],
    /// Each height, row by row, one more each way than there are cells.
    pub heights: Vec<f32>,
    /// A bit for each cell, row by row, set where there's a hole.
    pub holes: Vec<u8>,
}

impl TerrainData {
    /// Flat land of `chunks` chunks each way, with heights `cell` apart.
    #[must_use]
    pub fn flat(chunks: [u32; 2], cell: f32) -> Self {
        let cells = chunks.map(|count| count * CHUNK_CELLS);
        let samples = (cells[0] + 1) as usize * (cells[1] + 1) as usize;
        Self {
            cell,
            cells,
            heights: vec![0.0; samples],
            holes: vec![0; (cells[0] as usize * cells[1] as usize).div_ceil(8)],
        }
    }

    /// How many heights there are across a row.
    #[must_use]
    pub const fn across(&self) -> u32 {
        self.cells[0] + 1
    }

    /// The height at sample `x`, `y`, kept to the edge.
    #[must_use]
    pub fn height(&self, x: u32, y: u32) -> f32 {
        let x = x.min(self.cells[0]) as usize;
        let y = y.min(self.cells[1]) as usize;
        self.heights
            .get(y * self.across() as usize + x)
            .copied()
            .unwrap_or(0.0)
    }

    /// Whether cell `x`, `y` is a hole.
    #[must_use]
    pub fn is_hole(&self, x: u32, y: u32) -> bool {
        let at = y as usize * self.cells[0] as usize + x as usize;
        self.holes
            .get(at / 8)
            .is_some_and(|byte| byte >> (at % 8) & 1 != 0)
    }

    /// Makes cell `x`, `y` a hole, or fills it.
    pub fn set_hole(&mut self, x: u32, y: u32, hole: bool) {
        let at = y as usize * self.cells[0] as usize + x as usize;
        if let Some(byte) = self.holes.get_mut(at / 8) {
            let bit = 1 << (at % 8);
            *byte = if hole { *byte | bit } else { *byte & !bit };
        }
    }

    /// How many chunks there are each way.
    #[must_use]
    pub const fn chunks(&self) -> [u32; 2] {
        [
            self.cells[0].div_ceil(CHUNK_CELLS),
            self.cells[1].div_ceil(CHUNK_CELLS),
        ]
    }
}

/// Land in a level. Sheets stack, so that one can roof another, and between them they make caves,
/// overhangs and bridges.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TerrainSheet {
    /// Its heights.
    pub data: TerrainData,
    /// Where its low corner is, with what its heights are measured from.
    pub origin: Vec3,
    /// Whether it faces down, so that it's solid above and not below.
    pub facing_down: bool,
    /// How thick it is, or 0 for as far as it goes.
    pub thickness: f32,
    /// Its material in [`crate::Map::textures`].
    pub texture: u32,
    /// The first of its chunks' faces in [`crate::Map::faces`], row by row.
    pub first_face: u32,
}

impl TerrainSheet {
    /// How many chunks it has.
    #[must_use]
    pub const fn chunk_count(&self) -> u32 {
        let [x, y] = self.data.chunks();
        x * y
    }

    /// The world place of sample `x`, `y`.
    #[must_use]
    pub fn sample(&self, x: u32, y: u32) -> Vec3 {
        #[expect(clippy::cast_precision_loss, reason = "a land's size")]
        let across = Vec3::new(x as f32, y as f32, 0.0) * self.data.cell;
        self.origin + across + Vec3::new(0.0, 0.0, self.data.height(x, y))
    }

    /// Which way the land faces at sample `x`, `y`, by what's either side of it.
    #[must_use]
    pub fn normal(&self, x: u32, y: u32) -> Vec3 {
        let data = &self.data;
        let (left, right) = (x.saturating_sub(1), (x + 1).min(data.cells[0]));
        let (near, far) = (y.saturating_sub(1), (y + 1).min(data.cells[1]));
        #[expect(clippy::cast_precision_loss, reason = "one or two cells")]
        let (wide, deep) = (
            (right - left) as f32 * data.cell,
            (far - near) as f32 * data.cell,
        );
        let slope_x = (data.height(right, y) - data.height(left, y)) / wide.max(1.0);
        let slope_y = (data.height(x, far) - data.height(x, near)) / deep.max(1.0);
        let up = Vec3::new(-slope_x, -slope_y, 1.0);
        let facing = up * (1.0 / up.length());
        if self.facing_down {
            facing * -1.0
        } else {
            facing
        }
    }

    /// The two triangles of cell `x`, `y`, split from its low corner to its high one, wound to face
    /// the way the sheet does.
    #[must_use]
    pub fn cell_triangles(&self, x: u32, y: u32) -> [[Vec3; 3]; 2] {
        let a = self.sample(x, y);
        let b = self.sample(x + 1, y);
        let c = self.sample(x + 1, y + 1);
        let d = self.sample(x, y + 1);
        if self.facing_down {
            [[a, c, b], [a, d, c]]
        } else {
            [[a, b, c], [a, c, d]]
        }
    }

    /// The height of the land over a place, on its triangles, and `None` off the sheet or over a
    /// hole.
    #[must_use]
    pub fn height_at(&self, x: f32, y: f32) -> Option<f32> {
        let cell = self.data.cell;
        let (across, down) = ((x - self.origin.x) / cell, (y - self.origin.y) / cell);
        let [cells_x, cells_y] = self.data.cells;
        #[expect(clippy::cast_precision_loss, reason = "a land's size")]
        if across < 0.0 || down < 0.0 || across >= cells_x as f32 || down >= cells_y as f32 {
            return None;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "checked to be on the sheet"
        )]
        let (column, row) = (across as u32, down as u32);
        if self.data.is_hole(column, row) {
            return None;
        }
        let (a, c) = (
            self.data.height(column, row),
            self.data.height(column + 1, row + 1),
        );
        #[expect(clippy::cast_precision_loss, reason = "a land's size")]
        let (along, up) = (across - column as f32, down - row as f32);
        // A cell's split from its low corner to its high one.
        let height = if along >= up {
            let b = self.data.height(column + 1, row);
            a + (b - a) * along + (c - b) * up
        } else {
            let d = self.data.height(column, row + 1);
            a + (c - d) * along + (d - a) * up
        };
        Some(height + self.origin.z)
    }

    /// How high the land is at a place, holes or not, and which way it faces, kept to the sheet.
    #[must_use]
    pub fn surface_at(&self, x: f32, y: f32) -> (f32, Vec3) {
        let cell = self.data.cell;
        #[expect(clippy::cast_precision_loss, reason = "a land's size")]
        let (last_x, last_y) = (self.data.cells[0] as f32, self.data.cells[1] as f32);
        let across = ((x - self.origin.x) / cell).clamp(0.0, last_x);
        let down = ((y - self.origin.y) / cell).clamp(0.0, last_y);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "kept to the sheet"
        )]
        let (column, row) = (across as u32, down as u32);
        #[expect(clippy::cast_precision_loss, reason = "a land's size")]
        let (along, up) = (across - column as f32, down - row as f32);
        let height = |x, y| self.data.height(x, y);
        let (a, b) = (height(column, row), height(column + 1, row));
        let (c, d) = (height(column + 1, row + 1), height(column, row + 1));
        let near = a + (b - a) * along;
        let far = d + (c - d) * along;
        let blend = |one: Vec3, two: Vec3, share: f32| one + (two - one) * share;
        let facing = blend(
            blend(
                self.normal(column, row),
                self.normal(column + 1, row),
                along,
            ),
            blend(
                self.normal(column, row + 1),
                self.normal(column + 1, row + 1),
                along,
            ),
            up,
        );
        (
            near + (far - near) * up + self.origin.z,
            facing * (1.0 / facing.length().max(f32::EPSILON)),
        )
    }

    /// Whether `point` is inside the land: under it, or over it if it faces down, and within its
    /// thickness.
    #[must_use]
    pub fn holds(&self, point: Vec3) -> bool {
        let Some(height) = self.height_at(point.x, point.y) else {
            return false;
        };
        let inside = if self.facing_down {
            point.z - height
        } else {
            height - point.z
        };
        inside > 0.0 && (self.thickness <= 0.0 || inside < self.thickness)
    }

    /// Whether a box at `point` is in the land: a corner of its feet in ground, or of its head in
    /// a roof.
    #[must_use]
    pub fn holds_box(&self, point: Vec3, (mins, maxs): (Vec3, Vec3)) -> bool {
        let z = if self.facing_down { maxs.z } else { mins.z };
        [
            (mins.x, mins.y),
            (maxs.x, mins.y),
            (maxs.x, maxs.y),
            (mins.x, maxs.y),
        ]
        .iter()
        .any(|(x, y)| self.holds(point + Vec3::new(*x, *y, z)))
    }

    /// The low and high height of the chunk at `x`, `y`.
    #[must_use]
    pub fn chunk_heights(&self, x: u32, y: u32) -> [f32; 2] {
        let (mut low, mut high) = (f32::MAX, f32::MIN);
        let first = [x * CHUNK_CELLS, y * CHUNK_CELLS];
        for row in first[1]..=(first[1] + CHUNK_CELLS).min(self.data.cells[1]) {
            for column in first[0]..=(first[0] + CHUNK_CELLS).min(self.data.cells[0]) {
                let height = self.data.height(column, row);
                low = low.min(height);
                high = high.max(height);
            }
        }
        [low + self.origin.z, high + self.origin.z]
    }
}
