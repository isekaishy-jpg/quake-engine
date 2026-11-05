//! Packing rectangles into the lightmap atlas.

use std::collections::BTreeMap;

/// The atlas starts at this size and doubles until everything fits.
const FIRST_SIZE: u32 = 512;

/// The smallest square a tile's light can take.
const SMALLEST_TILE: u32 = 32;

/// Rectangles packed in rows, tallest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Atlas {
    /// The side of the square.
    pub size: u32,
    /// Each rectangle's top-left corner, in the order given.
    pub positions: Vec<[u32; 2]>,
}

impl Atlas {
    /// Packs `sizes`, as width and height, into the smallest square of up to
    /// `limit`, or `None` if they don't fit.
    #[must_use]
    pub fn pack(sizes: &[[u32; 2]], limit: u32) -> Option<Self> {
        Self::pack_from(sizes, limit, FIRST_SIZE)
    }

    /// Packs `sizes` as [`Atlas::pack`] does, into as small a square as a
    /// tile's light can have.
    #[must_use]
    pub fn pack_tile(sizes: &[[u32; 2]], limit: u32) -> Option<Self> {
        Self::pack_from(sizes, limit, SMALLEST_TILE)
    }

    /// Packs `sizes` into the smallest square from `first` up to `limit`.
    fn pack_from(sizes: &[[u32; 2]], limit: u32, first: u32) -> Option<Self> {
        let mut order: Vec<usize> = (0..sizes.len()).collect();
        order.sort_by_key(|&index| std::cmp::Reverse((sizes[index][1], sizes[index][0])));
        let mut size = first.min(limit);
        loop {
            if let Some(positions) = Self::try_pack(sizes, &order, size) {
                return Some(Self { size, positions });
            }
            if size >= limit {
                return None;
            }
            size = (size * 2).min(limit);
        }
    }

    /// Packs into a square of `size`, in `order`.
    fn try_pack(sizes: &[[u32; 2]], order: &[usize], size: u32) -> Option<Vec<[u32; 2]>> {
        let mut positions = vec![[0, 0]; sizes.len()];
        let (mut x, mut y, mut row_height) = (0, 0, 0);
        for &index in order {
            let [width, height] = sizes[index];
            if width > size {
                return None;
            }
            if x + width > size {
                x = 0;
                y += row_height;
                row_height = 0;
            }
            if y + height > size {
                return None;
            }
            positions[index] = [x, y];
            x += width;
            row_height = row_height.max(height);
        }
        Some(positions)
    }
}

/// Squares of an atlas handed out and taken back. Each side is a power of two,
/// and a square splits into four, so freed neighbours join up again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AtlasSpace {
    /// The free squares of each side.
    free: BTreeMap<u32, Vec<[u32; 2]>>,
}

impl AtlasSpace {
    /// The space of an atlas `side` across, with a square `reserved` across in
    /// its corner already taken.
    #[must_use]
    pub fn new(side: u32, reserved: u32) -> Self {
        let mut space = Self::default();
        let mut size = reserved.max(1);
        while size < side {
            let squares = space.free.entry(size).or_default();
            squares.extend([[size, 0], [0, size], [size, size]]);
            size *= 2;
        }
        space
    }

    /// Takes a square `size` across, which has to be a power of two, and says
    /// where it is, or `None` if there's no room.
    pub fn take(&mut self, size: u32) -> Option<[u32; 2]> {
        let found = *self
            .free
            .range(size..)
            .find(|(_, squares)| !squares.is_empty())?
            .0;
        let [x, y] = self.free.get_mut(&found)?.pop()?;
        let mut side = found;
        while side > size {
            side /= 2;
            let squares = self.free.entry(side).or_default();
            squares.extend([[x + side, y], [x, y + side], [x + side, y + side]]);
        }
        Some([x, y])
    }

    /// Gives back the square at `at`, `size` across.
    pub fn give(&mut self, at: [u32; 2], size: u32) {
        let parent = size * 2;
        let corner = [at[0] / parent * parent, at[1] / parent * parent];
        let others: Vec<[u32; 2]> = [[0, 0], [size, 0], [0, size], [size, size]]
            .iter()
            .map(|[x, y]| [corner[0] + x, corner[1] + y])
            .filter(|square| *square != at)
            .collect();
        let squares = self.free.entry(size).or_default();
        if others.iter().all(|square| squares.contains(square)) {
            squares.retain(|square| !others.contains(square));
            self.give(corner, parent);
        } else {
            squares.push(at);
        }
    }
}
