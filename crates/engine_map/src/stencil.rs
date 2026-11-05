//! Ray stencils: invisible surfaces light shines through as visible rays
//! (proposal 0076).

use engine_core::math::Vec3;

use crate::live::along_path;
use crate::material::SlotImage;

/// Cells along each side of a stencil's occlusion grid.
pub const STENCIL_GRID: usize = 16;

/// An invisible surface, such as a stained glass window, that shapes visible
/// rays. Colour tints them, and alpha blocks them, so a frame cuts the light.
#[derive(Clone, Debug, PartialEq)]
pub struct RayStencil {
    /// One corner.
    pub corner: Vec3,
    /// The edge along the image's width.
    pub across: Vec3,
    /// The edge along the image's height.
    pub down: Vec3,
    /// Where the light is, or the direction to it for a sun.
    pub light: Vec3,
    /// Whether `light` is a direction.
    pub sun: bool,
    /// Points `light` moves through in turn, if it moves.
    pub path: Vec<Vec3>,
    /// How many seconds a trip round `path` takes.
    pub period: f32,
    /// How bright the rays are.
    pub strength: f32,
    /// How far they reach.
    pub reach: f32,
    /// The stencil's picture.
    pub image: SlotImage,
    /// Boxes of the picture tested for occlusion together, as left, top, right
    /// and bottom from 0 to 1.
    pub boxes: Vec<[f32; 4]>,
}

impl RayStencil {
    /// Where the light is at `time`.
    #[must_use]
    pub fn light_at(&self, time: f32) -> Vec3 {
        along_path(&self.path, self.period, time, self.light)
    }

    /// The middle of `area` on the surface.
    #[must_use]
    pub fn point_in(&self, area: [f32; 4]) -> Vec3 {
        let across = (area[0] + area[2]) * 0.5;
        let down = (area[1] + area[3]) * 0.5;
        self.corner + self.across * across + self.down * down
    }

    /// Fills `mask` from how open each box is, leaving the rest open.
    pub fn fill_mask(&self, open: &[f32], mask: &mut [f32; STENCIL_GRID * STENCIL_GRID]) {
        mask.fill(1.0);
        for (area, open) in self.boxes.iter().zip(open) {
            #[expect(clippy::cast_precision_loss, reason = "a small grid")]
            let cells = STENCIL_GRID as f32;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "boxes are between 0 and 1"
            )]
            let cell = |value: f32| (value.clamp(0.0, 1.0) * cells) as usize;
            for y in cell(area[1])..cell(area[3]).min(STENCIL_GRID) {
                for x in cell(area[0])..cell(area[2]).min(STENCIL_GRID) {
                    mask[y * STENCIL_GRID + x] = *open;
                }
            }
        }
    }
}

/// Finds the boxes a stencil's pieces make, from an image of lines.
///
/// Pieces are areas between black lines. A red line groups the pieces inside it
/// into one box for occlusion, so a window of a hundred panes can be tested as
/// ten boxes. With no lines the whole stencil is one box.
#[must_use]
pub fn stencil_boxes(width: usize, height: usize, rgba: &[u8]) -> Vec<[f32; 4]> {
    let pixel = |x: usize, y: usize| {
        let at = (y * width + x) * 4;
        (
            rgba.get(at).copied().unwrap_or(0),
            rgba.get(at + 1).copied().unwrap_or(0),
            rgba.get(at + 2).copied().unwrap_or(0),
        )
    };
    let is_red = |(red, green, blue): (u8, u8, u8)| red > 160 && green < 96 && blue < 96;
    // Areas are what red lines enclose.
    let mut seen = vec![false; width * height];
    let mut boxes = Vec::new();
    for start_y in 0..height {
        for start_x in 0..width {
            if seen[start_y * width + start_x] || is_red(pixel(start_x, start_y)) {
                continue;
            }
            let mut stack = vec![(start_x, start_y)];
            seen[start_y * width + start_x] = true;
            let (mut left, mut top, mut right, mut bottom) = (start_x, start_y, start_x, start_y);
            while let Some((x, y)) = stack.pop() {
                left = left.min(x);
                top = top.min(y);
                right = right.max(x);
                bottom = bottom.max(y);
                let mut visit = |next_x: usize, next_y: usize| {
                    let at = next_y * width + next_x;
                    if !seen[at] && !is_red(pixel(next_x, next_y)) {
                        seen[at] = true;
                        stack.push((next_x, next_y));
                    }
                };
                if x > 0 {
                    visit(x - 1, y);
                }
                if y > 0 {
                    visit(x, y - 1);
                }
                if x + 1 < width {
                    visit(x + 1, y);
                }
                if y + 1 < height {
                    visit(x, y + 1);
                }
            }
            #[expect(clippy::cast_precision_loss, reason = "image sizes are small")]
            let (w, h) = (width as f32, height as f32);
            #[expect(clippy::cast_precision_loss, reason = "image sizes are small")]
            boxes.push([
                left as f32 / w,
                top as f32 / h,
                (right + 1) as f32 / w,
                (bottom + 1) as f32 / h,
            ]);
        }
    }
    if boxes.is_empty() {
        boxes.push([0.0, 0.0, 1.0, 1.0]);
    }
    boxes
}
