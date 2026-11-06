//! Fitting convex pieces to a mesh, for collision (proposal 0111).
//!
//! Collision goes through the map's trees as brushes, and a brush is convex, so a
//! well has to become a ring of convex pieces. The mesh is turned into voxels,
//! what can't be reached from outside is filled, and what's left is cut until
//! each piece is nearly as full as the planes round it.

use crate::geometry::{Point, add_scaled, dot, normalized};
use crate::mesh::Mesh;

/// A convex piece, as the planes round it, in the mesh's own space. Each is a
/// direction outwards and how far along it the plane is.
pub type ConvexPiece = Vec<(Point, f64)>;

/// The fewest voxels across a mesh's longest side.
const VOXELS_ACROSS: f64 = 48.0;

/// The smallest a voxel gets, in world units: well under what a player notices.
const SMALLEST_VOXEL: f64 = 1.0;

/// How much of what a piece's planes hold can be empty.
const MOST_EMPTY: f64 = 0.12;

/// How many times a piece can be cut.
const MOST_CUTS: u32 = 7;

/// The fewest voxels worth cutting.
const FEWEST_VOXELS: usize = 12;

/// The directions a piece's planes face. Axes and diagonals fit most shapes, and
/// the ones between them fit what's round about an upright axis.
fn plane_directions() -> Vec<Point> {
    let mut out = Vec::new();
    for x in -1..=1_i32 {
        for y in -1..=1_i32 {
            for z in -1..=1_i32 {
                let direction = [f64::from(x), f64::from(y), f64::from(z)];
                if let Some(direction) = normalized(direction) {
                    out.push(direction);
                }
            }
        }
    }
    for eighth in 0..8 {
        let angle = (f64::from(eighth) + 0.5) * std::f64::consts::FRAC_PI_4;
        let (y, x) = angle.sin_cos();
        out.push([x, y, 0.0]);
    }
    out
}

/// A grid of voxels over a mesh.
struct Grid {
    /// The corner of its first voxel.
    low: Point,
    /// The side of a voxel.
    size: f64,
    /// How many there are along each axis.
    counts: [usize; 3],
    /// Whether each is solid.
    solid: Vec<bool>,
}

impl Grid {
    /// The number of the voxel at `cell`.
    const fn at(&self, cell: [usize; 3]) -> usize {
        (cell[2] * self.counts[1] + cell[1]) * self.counts[0] + cell[0]
    }

    /// The cell holding `point`, held to the grid.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "held to the grid"
    )]
    fn cell_of(&self, point: Point) -> [usize; 3] {
        let mut cell = [0; 3];
        for (axis, slot) in cell.iter_mut().enumerate() {
            let place = ((point[axis] - self.low[axis]) / self.size).floor().max(0.0);
            *slot = (place as usize).min(self.counts[axis] - 1);
        }
        cell
    }

    /// Marks what a triangle touches, by stepping across it.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "small"
    )]
    fn mark(&mut self, corners: [Point; 3]) {
        let [a, b, c] = corners;
        let length = |p: Point, q: Point| {
            let gap = add_scaled(p, q, -1.0);
            dot(gap, gap).sqrt()
        };
        let longest = length(a, b).max(length(b, c)).max(length(c, a));
        let steps = (longest / (self.size * 0.5)).ceil().max(1.0) as u32;
        for i in 0..=steps {
            for j in 0..=steps - i {
                let (u, v) = (
                    f64::from(i) / f64::from(steps),
                    f64::from(j) / f64::from(steps),
                );
                let mut point = add_scaled(a, add_scaled(b, a, -1.0), u);
                point = add_scaled(point, add_scaled(c, a, -1.0), v);
                let number = self.at(self.cell_of(point));
                self.solid[number] = true;
            }
        }
    }

    /// Fills what can't be reached from the grid's edge.
    fn fill(&mut self) {
        let [nx, ny, nz] = self.counts;
        let mut outside = vec![false; self.solid.len()];
        let mut next = Vec::new();
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let edge = x == 0 || y == 0 || z == 0;
                    let edge = edge || x == nx - 1 || y == ny - 1 || z == nz - 1;
                    let number = self.at([x, y, z]);
                    if edge && !self.solid[number] {
                        outside[number] = true;
                        next.push([x, y, z]);
                    }
                }
            }
        }
        while let Some(cell) = next.pop() {
            for neighbour in neighbours(cell, self.counts) {
                let number = self.at(neighbour);
                if !self.solid[number] && !outside[number] {
                    outside[number] = true;
                    next.push(neighbour);
                }
            }
        }
        for (solid, out) in self.solid.iter_mut().zip(outside) {
            *solid = !out;
        }
    }

    /// The corners of the cell at `cell`.
    fn corners(&self, cell: [usize; 3]) -> [Point; 8] {
        let mut out = [[0.0; 3]; 8];
        #[expect(clippy::cast_precision_loss, reason = "grids are small")]
        for (number, corner) in out.iter_mut().enumerate() {
            for (axis, part) in corner.iter_mut().enumerate() {
                let step = cell[axis] + ((number >> axis) & 1);
                *part = self.low[axis] + step as f64 * self.size;
            }
        }
        out
    }
}

/// The cells next to `cell`, on the grid.
fn neighbours(cell: [usize; 3], counts: [usize; 3]) -> impl Iterator<Item = [usize; 3]> {
    (0..3).flat_map(move |axis| {
        let lower = (cell[axis] > 0).then(|| {
            let mut moved = cell;
            moved[axis] -= 1;
            moved
        });
        let higher = (cell[axis] + 1 < counts[axis]).then(|| {
            let mut moved = cell;
            moved[axis] += 1;
            moved
        });
        lower.into_iter().chain(higher)
    })
}

/// The voxels of `mesh`, or `None` if it has no size.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "grids are small"
)]
fn voxels_of(mesh: &Mesh) -> Option<Grid> {
    let points = mesh.parts.iter().flat_map(|part| &part.points);
    let (mut low, mut high) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for point in points {
        for axis in 0..3 {
            low[axis] = low[axis].min(point[axis]);
            high[axis] = high[axis].max(point[axis]);
        }
    }
    let longest = (0..3).map(|axis| high[axis] - low[axis]).fold(0.0, f64::max);
    if !longest.is_finite() || longest <= 0.0 {
        return None;
    }
    let size = (longest / VOXELS_ACROSS).max(SMALLEST_VOXEL);
    // A voxel's room all round, so what's outside can get all the way round.
    let low = low.map(|part| part - size);
    let counts = [0, 1, 2].map(|axis| ((high[axis] - low[axis]) / size).ceil() as usize + 2);
    let total = counts[0] * counts[1] * counts[2];
    let mut grid = Grid {
        low,
        size,
        counts,
        solid: vec![false; total],
    };
    for part in &mesh.parts {
        for triangle in &part.triangles {
            grid.mark(triangle.map(|corner| part.points[corner as usize]));
        }
    }
    grid.fill();
    Some(grid)
}

/// `cells` split into what's joined up.
fn joined_up(grid: &Grid, cells: &[[usize; 3]]) -> Vec<Vec<[usize; 3]>> {
    let mut member = vec![false; grid.solid.len()];
    for cell in cells {
        member[grid.at(*cell)] = true;
    }
    let mut pieces = Vec::new();
    for cell in cells {
        if !member[grid.at(*cell)] {
            continue;
        }
        member[grid.at(*cell)] = false;
        let mut piece = vec![*cell];
        let mut next = vec![*cell];
        while let Some(at) = next.pop() {
            for neighbour in neighbours(at, grid.counts) {
                let number = grid.at(neighbour);
                if member[number] {
                    member[number] = false;
                    piece.push(neighbour);
                    next.push(neighbour);
                }
            }
        }
        pieces.push(piece);
    }
    pieces
}

/// The low and high cells of `cells`.
fn cell_bounds(cells: &[[usize; 3]]) -> ([usize; 3], [usize; 3]) {
    let mut low = [usize::MAX; 3];
    let mut high = [0; 3];
    for cell in cells {
        for axis in 0..3 {
            low[axis] = low[axis].min(cell[axis]);
            high[axis] = high[axis].max(cell[axis]);
        }
    }
    (low, high)
}

/// The planes round `cells`, as how far each direction reaches.
fn reaches(grid: &Grid, cells: &[[usize; 3]], directions: &[Point]) -> Vec<f64> {
    let mut far = vec![f64::NEG_INFINITY; directions.len()];
    for cell in cells {
        for corner in grid.corners(*cell) {
            for (reach, direction) in far.iter_mut().zip(directions) {
                *reach = reach.max(dot(corner, *direction));
            }
        }
    }
    far
}

/// How much of what `reach` holds isn't in `cells`, as a share.
#[expect(clippy::cast_precision_loss, reason = "grids are small")]
fn emptiness(grid: &Grid, cells: &[[usize; 3]], directions: &[Point], far: &[f64]) -> f64 {
    let (low, high) = cell_bounds(cells);
    let half = grid.size * 0.5;
    let mut inside = 0_usize;
    for z in low[2]..=high[2] {
        for y in low[1]..=high[1] {
            for x in low[0]..=high[0] {
                let [a, ..] = grid.corners([x, y, z]);
                let middle = a.map(|part| part + half);
                let held = directions
                    .iter()
                    .zip(far)
                    .all(|(direction, reach)| dot(middle, *direction) <= *reach);
                inside += usize::from(held);
            }
        }
    }
    1.0 - cells.len() as f64 / inside.max(1) as f64
}

/// The volume of the boxes round `cells`, cut at `at` along `axis`.
fn boxes_after_cut(cells: &[[usize; 3]], axis: usize, at: usize) -> usize {
    let (lower, higher): (Vec<_>, Vec<_>) = cells.iter().partition(|cell| cell[axis] < at);
    let volume = |cells: &[[usize; 3]]| {
        if cells.is_empty() {
            return 0;
        }
        let (low, high) = cell_bounds(cells);
        (0..3).map(|axis| high[axis] - low[axis] + 1).product()
    };
    volume(&lower) + volume(&higher)
}

/// The best place to cut `cells`: where the boxes round its halves hold least.
fn best_cut(cells: &[[usize; 3]]) -> Option<(usize, usize)> {
    let (low, high) = cell_bounds(cells);
    let mut best: Option<(usize, usize, usize)> = None;
    for axis in 0..3 {
        let span = high[axis] - low[axis];
        if span < 1 {
            continue;
        }
        for step in 1..8 {
            let at = low[axis] + 1 + (span - 1) * step / 8;
            let volume = boxes_after_cut(cells, axis, at);
            if best.is_none_or(|(_, _, known)| volume < known) {
                best = Some((axis, at, volume));
            }
        }
    }
    best.map(|(axis, at, _)| (axis, at))
}

/// Cuts `cells` until each piece fits, and adds each to `out`.
fn cut(
    grid: &Grid,
    cells: &[[usize; 3]],
    cuts: u32,
    directions: &[Point],
    out: &mut Vec<ConvexPiece>,
) {
    for piece in joined_up(grid, cells) {
        let far = reaches(grid, &piece, directions);
        let fits = cuts >= MOST_CUTS
            || piece.len() < FEWEST_VOXELS
            || emptiness(grid, &piece, directions, &far) <= MOST_EMPTY;
        let split = (!fits).then(|| best_cut(&piece)).flatten();
        if let Some((axis, at)) = split {
            let (lower, higher): (Vec<_>, Vec<_>) =
                piece.iter().partition(|cell| cell[axis] < at);
            cut(grid, &lower, cuts + 1, directions, out);
            cut(grid, &higher, cuts + 1, directions, out);
        } else {
            out.push(directions.iter().copied().zip(far).collect());
        }
    }
}

/// Convex pieces that fit `mesh`, or none if it has no size.
#[must_use]
pub fn fit_convex(mesh: &Mesh) -> Vec<ConvexPiece> {
    let Some(grid) = voxels_of(mesh) else {
        return Vec::new();
    };
    let [nx, ny, nz] = grid.counts;
    let mut cells = Vec::new();
    for z in 0..nz {
        for y in 0..ny {
            for x in 0..nx {
                if grid.solid[grid.at([x, y, z])] {
                    cells.push([x, y, z]);
                }
            }
        }
    }
    let directions = plane_directions();
    let mut pieces = Vec::new();
    cut(&grid, &cells, 0, &directions, &mut pieces);
    pieces
}
