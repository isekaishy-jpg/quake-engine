//! Packing rectangles into the lightmap atlas.

/// The atlas starts at this size and doubles until everything fits.
const FIRST_SIZE: u32 = 512;

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
        let mut order: Vec<usize> = (0..sizes.len()).collect();
        order.sort_by_key(|&index| std::cmp::Reverse((sizes[index][1], sizes[index][0])));
        let mut size = FIRST_SIZE.min(limit);
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
