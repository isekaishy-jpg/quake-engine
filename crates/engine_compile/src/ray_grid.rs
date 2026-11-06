//! Finding what a ray might touch, without asking everything in the map.
//!
//! A bake sends out a great many rays. Asking each of them about every mesh and
//! pane of glass makes a map's cost grow with the square of its size, so these
//! are kept by where they stand, and a ray asks only about what's near it.

use std::collections::HashMap;

use crate::geometry::Point;

/// How wide a cell is, about the size of a room or two.
const CELL: f64 = 1024.0;

/// A column of cells, from above.
type Cell = [i64; 2];

/// The cell of `x` and `y`.
#[expect(clippy::cast_possible_truncation, reason = "worlds are small")]
fn cell_of(x: f64, y: f64) -> Cell {
    [(x / CELL).floor() as i64, (y / CELL).floor() as i64]
}

/// What's in a map by where it stands, as indices into a list kept elsewhere.
#[derive(Clone, Debug, Default)]
pub struct RayGrid {
    /// What touches each cell.
    cells: HashMap<Cell, Vec<u32>>,
    /// The lowest cell each item touches.
    first: Vec<Cell>,
}

impl RayGrid {
    /// A grid of items, each given by its low and high corners.
    #[must_use]
    pub fn new(bounds: impl Iterator<Item = (Point, Point)>) -> Self {
        let mut grid = Self::default();
        for (index, (low, high)) in bounds.enumerate() {
            let index = u32::try_from(index).unwrap_or(u32::MAX);
            let start = cell_of(low[0], low[1]);
            let end = cell_of(high[0], high[1]);
            grid.first.push(start);
            for y in start[1]..=end[1] {
                for x in start[0]..=end[0] {
                    grid.cells.entry([x, y]).or_default().push(index);
                }
            }
        }
        grid
    }

    /// Calls `visit` once for each item near the line from `a` to `b`, until it
    /// returns true, and returns whether it did.
    pub fn any_near(&self, a: Point, b: Point, mut visit: impl FnMut(usize) -> bool) -> bool {
        if self.cells.is_empty() {
            return false;
        }
        let start = cell_of(a[0].min(b[0]), a[1].min(b[1]));
        let end = cell_of(a[0].max(b[0]), a[1].max(b[1]));
        for y in start[1]..=end[1] {
            for x in start[0]..=end[0] {
                let Some(items) = self.cells.get(&[x, y]) else {
                    continue;
                };
                for item in items {
                    let index = *item as usize;
                    // Something across several cells counts in the first of
                    // them that the line's bounds reach.
                    let first = self.first[index];
                    if [x, y] == [first[0].max(start[0]), first[1].max(start[1])] && visit(index) {
                        return true;
                    }
                }
            }
        }
        false
    }
}
