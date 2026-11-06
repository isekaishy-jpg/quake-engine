//! The engine's own look: fonts, frames and moving pictures (proposal 0117).
//!
//! All of it comes from a game's content, so a game with none of it goes on
//! drawing what it always did.

use std::collections::HashMap;

use engine_core::canvas::{Canvas, Image};
use fontdue::{Font, FontSettings};

/// The first and last characters a font bakes, which take in Western languages.
const CHARACTERS: std::ops::RangeInclusive<u32> = 0x20..=0xff;

/// The widest a font's picture gets.
const ATLAS_WIDTH: u32 = 1024;

/// The gap round each character, so filtering doesn't reach the next.
const GLYPH_GAP: u32 = 2;

/// How a font looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontStyle {
    /// For reading.
    Regular,
    /// For what stands out.
    Bold,
    /// For titles.
    Display,
}

/// Where a character is in a font's picture, in pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    /// Its left, top, right and bottom edges, in the picture.
    pub source: [f32; 4],
    /// How far right of the pen its left edge is.
    pub left: f32,
    /// How far below the line's top its top edge is.
    pub top: f32,
    /// How far the pen moves on.
    pub advance: f32,
}

/// A font baked at one size.
#[derive(Clone, Debug, PartialEq)]
pub struct BakedFont {
    /// The name its picture goes by.
    pub picture: String,
    /// Its height in pixels.
    pub line_height: f32,
    /// Each character.
    pub glyphs: HashMap<char, Glyph>,
}

/// What can tell how wide a piece of text is.
pub trait TextMeasure {
    /// How far the pen moves for `character`.
    fn advance(&self, character: char) -> f32;
}

impl TextMeasure for BakedFont {
    fn advance(&self, character: char) -> f32 {
        self.glyphs
            .get(&character)
            .map_or(0.0, |glyph| glyph.advance)
    }
}

/// How wide `text` is.
#[must_use]
pub fn text_width(font: &dyn TextMeasure, text: &str) -> f32 {
    text.chars().map(|character| font.advance(character)).sum()
}

/// `text` as lines no wider than `width`, broken between words. A word too wide
/// for a line of its own is broken where it has to be.
#[must_use]
pub fn wrapped(font: &dyn TextMeasure, text: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            let joined = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if text_width(font, &joined) <= width {
                line = joined;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            for character in word.chars() {
                line.push(character);
                if text_width(font, &line) > width && line.chars().count() > 1 {
                    let last = line.pop();
                    lines.push(std::mem::take(&mut line));
                    line.extend(last);
                }
            }
        }
        lines.push(line);
    }
    lines
}

/// Bakes `font` at `pixels`, and returns it with its picture.
fn bake(font: &Font, name: String, pixels: f32) -> (BakedFont, Image) {
    let mut placed = Vec::new();
    let (mut x, mut y, mut row) = (GLYPH_GAP, GLYPH_GAP, 0);
    let metrics = font.horizontal_line_metrics(pixels);
    let ascent = metrics.map_or(pixels, |metrics| metrics.ascent);
    let line_height = metrics.map_or(pixels * 1.2, |metrics| metrics.new_line_size);
    for code in CHARACTERS {
        let Some(character) = char::from_u32(code) else {
            continue;
        };
        if font.lookup_glyph_index(character) == 0 {
            continue;
        }
        let (shape, coverage) = font.rasterize(character, pixels);
        let (width, height) = (
            u32::try_from(shape.width).unwrap_or(0),
            u32::try_from(shape.height).unwrap_or(0),
        );
        if x + width + GLYPH_GAP > ATLAS_WIDTH {
            x = GLYPH_GAP;
            y += row + GLYPH_GAP;
            row = 0;
        }
        placed.push((character, shape, coverage, x, y));
        x += width + GLYPH_GAP;
        row = row.max(height);
    }
    let height = (y + row + GLYPH_GAP).next_power_of_two();
    let mut rgba = vec![0; (ATLAS_WIDTH * height * 4) as usize];
    let mut glyphs = HashMap::with_capacity(placed.len());
    for (character, shape, coverage, x, y) in placed {
        let width = shape.width.max(1);
        for (at, alpha) in coverage.iter().enumerate() {
            let column = x as usize + at % width;
            let line = y as usize + at / width;
            let pixel = (line * ATLAS_WIDTH as usize + column) * 4;
            rgba[pixel..pixel + 4].copy_from_slice(&[255, 255, 255, *alpha]);
        }
        #[expect(clippy::cast_precision_loss, reason = "pictures are small")]
        let (left, top) = (x as f32, y as f32);
        #[expect(clippy::cast_precision_loss, reason = "characters are small")]
        let (across, down, rise) = (shape.width as f32, shape.height as f32, shape.height as f32);
        #[expect(clippy::cast_precision_loss, reason = "characters are small")]
        let (bearing, below) = (shape.xmin as f32, shape.ymin as f32);
        glyphs.insert(
            character,
            Glyph {
                source: [left, top, left + across, top + down],
                left: bearing,
                top: ascent - rise - below,
                advance: shape.advance_width,
            },
        );
    }
    let picture = Image {
        width: ATLAS_WIDTH,
        height,
        rgba,
    };
    let baked = BakedFont {
        picture: name,
        line_height,
        glyphs,
    };
    (baked, picture)
}

/// A piece of an atlas of frames and icons.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KitPiece {
    /// Its left, top, width and height in the atlas, in pixels.
    pub rect: [f32; 4],
    /// How far in the edges of a frame are, in pixels of the atlas, as left, top,
    /// right and bottom. All zeros for something that isn't a frame.
    pub slices: [f32; 4],
}

/// Frames and icons.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiKit {
    /// The picture they're on.
    pub picture: String,
    /// How many of its pixels make a unit of the kit.
    pub density: f32,
    /// Each piece, by name.
    pub pieces: HashMap<String, KitPiece>,
}

/// A picture that moves.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UiAnimation {
    /// The picture of each frame, with how many seconds it shows.
    pub frames: Vec<(String, f32)>,
    /// Whether it goes round.
    pub loops: bool,
}

impl UiAnimation {
    /// The picture to show `seconds` after it began, or `None` once it's over.
    #[must_use]
    pub fn frame_at(&self, seconds: f32) -> Option<&str> {
        let whole: f32 = self.frames.iter().map(|(_, time)| time).sum();
        if whole <= 0.0 {
            return None;
        }
        let mut left = if self.loops {
            seconds.rem_euclid(whole)
        } else {
            seconds
        };
        for (picture, time) in &self.frames {
            if left < *time {
                return Some(picture);
            }
            left -= time;
        }
        None
    }
}

/// Everything that makes up the engine's own look.
#[derive(Default)]
pub struct UiTheme {
    /// The fonts, ready to bake.
    fonts: HashMap<FontStyle, Font>,
    /// What's been baked, by style and pixels.
    baked: HashMap<(FontStyle, u32), BakedFont>,
    /// Frames and icons.
    pub kit: UiKit,
    /// Moving pictures, by name.
    pub animations: HashMap<String, UiAnimation>,
}

impl UiTheme {
    /// Adds the font in `bytes` for `style`. Returns whether it read.
    pub fn add_font(&mut self, style: FontStyle, bytes: &[u8]) -> bool {
        let font = Font::from_bytes(bytes, FontSettings::default());
        let read = font.is_ok();
        if let Ok(font) = font {
            self.fonts.insert(style, font);
        }
        read
    }

    /// Whether there's a font to draw with.
    #[must_use]
    pub fn has_fonts(&self) -> bool {
        !self.fonts.is_empty()
    }

    /// `style` at `pixels`, baked the first time it's asked for. The picture goes
    /// to `canvas`. A style that's missing uses another.
    pub fn font(
        &mut self,
        canvas: &mut Canvas<'_>,
        style: FontStyle,
        pixels: u32,
    ) -> Option<&BakedFont> {
        let style = if self.fonts.contains_key(&style) {
            style
        } else {
            *self.fonts.keys().min_by_key(|style| **style as u8)?
        };
        if !self.baked.contains_key(&(style, pixels)) {
            let font = self.fonts.get(&style)?;
            let name = format!("font:{style:?}:{pixels}");
            #[expect(clippy::cast_precision_loss, reason = "sizes are small")]
            let (baked, picture) = bake(font, name.clone(), pixels as f32);
            canvas.add_picture(&name, &picture);
            self.baked.insert((style, pixels), baked);
        }
        self.baked.get(&(style, pixels))
    }
}

/// Which colour text is, as red, green, blue and alpha.
pub type UiColor = [f32; 4];

/// Draws `text` with its top left at `x` and `y`, in units, `size` units tall.
pub fn draw_text(
    canvas: &mut Canvas<'_>,
    theme: &mut UiTheme,
    style: FontStyle,
    place: (f32, f32, f32),
    text: &str,
    color: UiColor,
) {
    let (x, y, size) = place;
    let scale = canvas.scale();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a size in pixels"
    )]
    let pixels = (size * scale).round().max(4.0) as u32;
    let Some(font) = theme.font(canvas, style, pixels) else {
        return;
    };
    let mut pen = x * scale;
    let top = y * scale;
    for character in text.chars() {
        if let Some(glyph) = font.glyphs.get(&character) {
            let [left, up, right, down] = glyph.source;
            let at = pen + glyph.left;
            let height = top + glyph.top;
            canvas.pixels(
                &font.picture,
                [left, up, right, down],
                [at, height, at + right - left, height + down - up],
                color,
            );
            pen += glyph.advance;
        }
    }
}

/// How wide `text` is in units, `size` units tall.
pub fn measure_text(
    canvas: &mut Canvas<'_>,
    theme: &mut UiTheme,
    style: FontStyle,
    size: f32,
    text: &str,
) -> f32 {
    let scale = canvas.scale();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a size in pixels"
    )]
    let pixels = (size * scale).round().max(4.0) as u32;
    theme
        .font(canvas, style, pixels)
        .map_or(0.0, |font| text_width(font, text) / scale)
}

/// The nine rectangles of a frame, as where each comes from in the atlas and
/// where it goes on the screen, in pixels. Corners stay as they are, and the rest
/// stretches. `density` is the atlas's pixels to a unit of the kit, and `scale`
/// the screen's.
#[must_use]
pub fn nine_slices(
    piece: &KitPiece,
    place: [f32; 4],
    density: f32,
    scale: f32,
) -> Vec<([f32; 4], [f32; 4])> {
    let [x, y, width, height] = piece.rect;
    let [left, top, right, bottom] = piece.slices;
    let [out_left, out_top, out_right, out_bottom] = place;
    let edge = scale / density.max(1.0);
    let across = [
        (x, x + left, out_left, out_left + left * edge),
        (
            x + left,
            x + width - right,
            out_left + left * edge,
            out_right - right * edge,
        ),
        (
            x + width - right,
            x + width,
            out_right - right * edge,
            out_right,
        ),
    ];
    let down = [
        (y, y + top, out_top, out_top + top * edge),
        (
            y + top,
            y + height - bottom,
            out_top + top * edge,
            out_bottom - bottom * edge,
        ),
        (
            y + height - bottom,
            y + height,
            out_bottom - bottom * edge,
            out_bottom,
        ),
    ];
    let mut out = Vec::with_capacity(9);
    for (source_top, source_bottom, top_edge, bottom_edge) in down {
        for (source_left, source_right, left_edge, right_edge) in across {
            if right_edge > left_edge && bottom_edge > top_edge {
                out.push((
                    [source_left, source_top, source_right, source_bottom],
                    [left_edge, top_edge, right_edge, bottom_edge],
                ));
            }
        }
    }
    out
}

/// Draws the frame called `name` over `place`, in units. It keeps its corners,
/// and goes no smaller than they are.
pub fn draw_frame(
    canvas: &mut Canvas<'_>,
    theme: &UiTheme,
    name: &str,
    place: [f32; 4],
    color: UiColor,
) {
    let Some(piece) = theme.kit.pieces.get(name) else {
        return;
    };
    let scale = canvas.scale();
    let pixels = place.map(|edge| edge * scale);
    for (source, target) in nine_slices(piece, pixels, theme.kit.density, scale) {
        canvas.pixels(&theme.kit.picture, source, target, color);
    }
}

/// Draws the icon or control called `name` over `place`, in units.
pub fn draw_piece(
    canvas: &mut Canvas<'_>,
    theme: &UiTheme,
    name: &str,
    place: [f32; 4],
    color: UiColor,
) {
    let Some(piece) = theme.kit.pieces.get(name) else {
        return;
    };
    let scale = canvas.scale();
    let [x, y, width, height] = piece.rect;
    canvas.pixels(
        &theme.kit.picture,
        [x, y, x + width, y + height],
        place.map(|edge| edge * scale),
        color,
    );
}

/// Draws `text` on a grid of `cell` units a character, `size` tall. Cultist Sans is
/// proportional, and a console needs columns, so each character sits in the middle
/// of a cell of its own.
pub fn draw_cells(
    canvas: &mut Canvas<'_>,
    theme: &mut UiTheme,
    place: (f32, f32, f32),
    size: f32,
    text: &str,
    color: UiColor,
) {
    let (x, y, cell) = place;
    let scale = canvas.scale();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a size in pixels"
    )]
    let pixels = (size * scale).round().max(4.0) as u32;
    let Some(font) = theme.font(canvas, FontStyle::Regular, pixels) else {
        return;
    };
    let top = y * scale;
    for (column, character) in (0_u16..).zip(text.chars()) {
        let Some(glyph) = font.glyphs.get(&character) else {
            continue;
        };
        let [left, up, right, down] = glyph.source;
        let middle = (x + f32::from(column) * cell + cell * 0.5) * scale;
        let at = (middle - glyph.advance * 0.5 + glyph.left).round();
        let height = top + glyph.top;
        canvas.pixels(
            &font.picture,
            [left, up, right, down],
            [at, height, at + right - left, height + down - up],
            color,
        );
    }
}
