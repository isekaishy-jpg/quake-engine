//! A line of text in the corner of the picture (proposal 0021).

/// The characters the built-in font has, in atlas order.
const CHARACTERS: &str = "0123456789FPS.";

/// Each character's rows, top first, with the high bit of five on the left.
const GLYPHS: [[u8; GLYPH_HEIGHT as usize]; 14] = [
    [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
    [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
    [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
    [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
    [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
    [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
    [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
    [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
    [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
    [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
    [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
    [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
    [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
];

/// A glyph's width in font pixels.
const GLYPH_WIDTH: u32 = 5;

/// A glyph's height in font pixels.
const GLYPH_HEIGHT: u32 = 7;

/// The space from one character to the next, in font pixels.
const ADVANCE: u32 = 6;

/// The width of an atlas cell.
const CELL: u32 = 8;

/// The atlas width: the glyphs, then the solid backing cell.
pub(crate) const ATLAS_WIDTH: u32 = 128;

/// The atlas height.
pub(crate) const ATLAS_HEIGHT: u32 = CELL;

/// The backing's cell in the atlas.
const BACKING_CELL: u32 = 15;

/// The most characters drawn.
pub(crate) const MAX_CHARACTERS: usize = 32;

/// The picture height that shows font pixels at 2 to 1.
const HEIGHT_PER_SCALE: u32 = 360;

/// The gap to the picture's edge, and around the text, in font pixels.
const MARGIN: u32 = 2;

/// Floats in one vertex: position and texture coordinates.
pub(crate) const VERTEX_FLOATS: usize = 4;

/// The most vertices a line of text needs.
pub(crate) const MAX_VERTICES: usize = (MAX_CHARACTERS + 1) * 6;

/// The font as RGBA bytes: white glyphs on nothing, and a dark backing cell.
#[must_use]
pub(crate) fn font_atlas() -> Vec<u8> {
    let (width, height) = (ATLAS_WIDTH as usize, ATLAS_HEIGHT as usize);
    let mut bytes = vec![0; width * height * 4];
    for (cell, rows) in GLYPHS.iter().enumerate() {
        for (y, row) in rows.iter().enumerate() {
            for x in 0..GLYPH_WIDTH as usize {
                if row & (1 << (GLYPH_WIDTH as usize - 1 - x)) != 0 {
                    let at = (y * width + cell * CELL as usize + x) * 4;
                    bytes[at..at + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            }
        }
    }
    for y in 0..height {
        for x in 0..CELL as usize {
            let at = (y * width + BACKING_CELL as usize * CELL as usize + x) * 4;
            bytes[at..at + 4].copy_from_slice(&[0, 0, 0, 160]);
        }
    }
    bytes
}

/// A rectangle in pixels, from the top left.
#[derive(Clone, Copy)]
struct Rect {
    /// The left edge.
    left: f32,
    /// The top edge.
    top: f32,
    /// The right edge.
    right: f32,
    /// The bottom edge.
    bottom: f32,
}

/// Appends two triangles for `rect`, showing an atlas cell.
fn push_quad(out: &mut Vec<f32>, rect: Rect, cell: u32, width: u32, size: (f32, f32)) {
    #[expect(
        clippy::cast_precision_loss,
        reason = "the atlas is far below 2^24 wide"
    )]
    let (u0, u1, v1) = (
        (cell * CELL) as f32 / ATLAS_WIDTH as f32,
        (cell * CELL + width) as f32 / ATLAS_WIDTH as f32,
        GLYPH_HEIGHT as f32 / ATLAS_HEIGHT as f32,
    );
    // Clip space has y up, as in the world shader.
    let x = |pixel: f32| pixel / size.0 * 2.0 - 1.0;
    let y = |pixel: f32| 1.0 - pixel / size.1 * 2.0;
    let corners = [
        [x(rect.left), y(rect.top), u0, 0.0],
        [x(rect.right), y(rect.top), u1, 0.0],
        [x(rect.right), y(rect.bottom), u1, v1],
        [x(rect.left), y(rect.top), u0, 0.0],
        [x(rect.right), y(rect.bottom), u1, v1],
        [x(rect.left), y(rect.bottom), u0, v1],
    ];
    out.extend(corners.into_iter().flatten());
}

/// The vertices of `text` at the top right of a picture of `size`, in groups
/// of [`VERTEX_FLOATS`]. Characters outside the font show as gaps.
#[must_use]
pub fn overlay_vertices(text: &str, size: (u32, u32)) -> Vec<f32> {
    let mut out = Vec::new();
    let count = u32::try_from(text.chars().count().min(MAX_CHARACTERS)).unwrap_or(0);
    if count == 0 || size.0 == 0 || size.1 == 0 {
        return out;
    }
    let scale = (size.1 / HEIGHT_PER_SCALE * 2).max(2);
    let text_width = (count * ADVANCE - 1) * scale;
    let pad = MARGIN * scale;
    let right = size.0.saturating_sub(pad * 2);
    let left = right.saturating_sub(text_width);
    let top = pad * 2;
    #[expect(
        clippy::cast_precision_loss,
        reason = "picture sizes are far below 2^24"
    )]
    {
        let picture = (size.0 as f32, size.1 as f32);
        let backing = Rect {
            left: left.saturating_sub(pad) as f32,
            top: pad as f32,
            right: (right + pad) as f32,
            bottom: (top + GLYPH_HEIGHT * scale + pad) as f32,
        };
        push_quad(&mut out, backing, BACKING_CELL, CELL, picture);
        for (position, character) in (0..count).zip(text.chars()) {
            let Some(cell) = CHARACTERS.find(character) else {
                continue;
            };
            let x = left + position * ADVANCE * scale;
            let rect = Rect {
                left: x as f32,
                top: top as f32,
                right: (x + GLYPH_WIDTH * scale) as f32,
                bottom: (top + GLYPH_HEIGHT * scale) as f32,
            };
            let cell = u32::try_from(cell).unwrap_or(0);
            push_quad(&mut out, rect, cell, GLYPH_WIDTH, picture);
        }
    }
    out
}
