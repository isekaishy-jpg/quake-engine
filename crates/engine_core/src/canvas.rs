//! Flat pictures and text over the view (proposal 0039).
//!
//! A [`Canvas`] measures in units of a low resolution screen, which it scales
//! to pixels, so what's drawn on it doesn't depend on the window's size.

/// The picture drawn for a solid colour.
pub const SOLID_PICTURE: u32 = 0;

/// The name of the font, a picture of 16 by 16 characters.
pub const FONT_PICTURE: &str = "conchars";

/// A character's size in canvas units.
pub const CHARACTER_SIZE: f32 = 8.0;

/// How many characters there are across the font.
const FONT_COLUMNS: u32 = 16;

/// White, which leaves a picture as it is.
const PLAIN: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// All of a picture.
const WHOLE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// Pixels to draw as a picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
    /// Four bytes a pixel, in RGBA order.
    pub rgba: Vec<u8>,
}

/// A picture the renderer holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Picture {
    /// Its number in the renderer.
    pub number: u32,
    /// Its width in pixels.
    pub width: u32,
    /// Its height in pixels.
    pub height: u32,
}

/// Where pictures come from, by name.
pub trait PictureSource {
    /// The picture called `name`, if there is one.
    fn picture(&mut self, name: &str) -> Option<Picture>;

    /// Keeps `image` as the picture called `name`, for what's made as it's needed.
    /// A source that can't take one leaves it.
    fn add(&mut self, _name: &str, _image: &Image) {}
}

/// What marks a line of text as someone talking, from `Con_Print`. It's not
/// drawn: the line is bright instead.
pub const TALK_MARK: char = '\u{1}';

/// A rectangle of a picture, drawn in pixels of the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    /// The picture.
    pub picture: u32,
    /// The left, top, right and bottom edges in pixels.
    pub place: [f32; 4],
    /// The same edges of the picture, from 0 to 1.
    pub source: [f32; 4],
    /// What the picture is multiplied by, as red, green, blue and alpha.
    pub color: [f32; 4],
}

/// A list of things to draw, in order.
pub struct Canvas<'a> {
    /// Where pictures come from.
    source: &'a mut dyn PictureSource,
    /// The size in pixels.
    size: (u32, u32),
    /// Pixels to a unit.
    scale: f32,
    /// What's been drawn.
    quads: Vec<Quad>,
    /// The font, once it's been looked for.
    font: Option<Option<Picture>>,
}

impl<'a> Canvas<'a> {
    /// A canvas of `size` pixels, with `scale` pixels to a unit.
    #[must_use]
    pub fn new(source: &'a mut dyn PictureSource, size: (u32, u32), scale: f32) -> Self {
        Self {
            source,
            size,
            scale,
            quads: Vec::new(),
            font: None,
        }
    }

    /// Pixels to a unit.
    #[must_use]
    pub const fn scale(&self) -> f32 {
        self.scale
    }

    /// Keeps `image` as the picture called `name`.
    pub fn add_picture(&mut self, name: &str, image: &Image) {
        self.source.add(name, image);
    }

    /// Draws part of the picture called `name` at `place`, both in pixels, as left,
    /// top, right and bottom. What needs exact pixels, such as text, goes by this.
    #[expect(clippy::cast_precision_loss, reason = "pictures are small")]
    pub fn pixels(&mut self, name: &str, source: [f32; 4], place: [f32; 4], color: [f32; 4]) {
        let Some(picture) = self.source.picture(name) else {
            return;
        };
        let (width, height) = (picture.width as f32, picture.height as f32);
        let [left, top, right, bottom] = source;
        self.quads.push(Quad {
            picture: picture.number,
            place,
            source: [left / width, top / height, right / width, bottom / height],
            color,
        });
    }

    /// The width in units.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "sizes are far below 2^24")]
    pub fn width(&self) -> f32 {
        self.size.0 as f32 / self.scale
    }

    /// The height in units.
    #[must_use]
    #[expect(clippy::cast_precision_loss, reason = "sizes are far below 2^24")]
    pub fn height(&self) -> f32 {
        self.size.1 as f32 / self.scale
    }

    /// The width and height of the picture called `name`, in units.
    #[expect(clippy::cast_precision_loss, reason = "pictures are small")]
    pub fn size_of(&mut self, name: &str) -> Option<(f32, f32)> {
        let picture = self.source.picture(name)?;
        Some((picture.width as f32, picture.height as f32))
    }

    /// Adds a rectangle of units.
    fn push(&mut self, picture: u32, place: [f32; 4], source: [f32; 4], color: [f32; 4]) {
        // Whole pixels keep a picture's own pixels even.
        let place = place.map(|edge| (edge * self.scale).round());
        self.quads.push(Quad {
            picture,
            place,
            source,
            color,
        });
    }

    /// Draws the picture called `name` with its top left corner at `x`, `y`.
    /// One that's missing draws nothing.
    #[expect(clippy::cast_precision_loss, reason = "pictures are small")]
    pub fn picture(&mut self, name: &str, x: f32, y: f32) {
        if let Some(picture) = self.source.picture(name) {
            let (width, height) = (picture.width as f32, picture.height as f32);
            self.push(picture.number, [x, y, x + width, y + height], WHOLE, PLAIN);
        }
    }

    /// Draws the picture called `name` stretched over a rectangle, of left,
    /// top, right and bottom edges.
    pub fn stretched(&mut self, name: &str, place: [f32; 4], alpha: f32) {
        if let Some(picture) = self.source.picture(name) {
            self.push(picture.number, place, WHOLE, [1.0, 1.0, 1.0, alpha]);
        }
    }

    /// Fills a rectangle with `color`, as red, green, blue and alpha from 0 to 1.
    pub fn fill(&mut self, place: [f32; 4], color: [f32; 4]) {
        self.push(SOLID_PICTURE, place, WHOLE, color);
    }

    /// The font.
    fn font(&mut self) -> Option<Picture> {
        if self.font.is_none() {
            self.font = Some(self.source.picture(FONT_PICTURE));
        }
        self.font.flatten()
    }

    /// Draws one character of the font, by its number.
    pub fn character(&mut self, x: f32, y: f32, code: u8) {
        let Some(font) = self.font() else {
            return;
        };
        #[expect(clippy::cast_precision_loss, reason = "both are at most 16")]
        let (column, row, step) = (
            (u32::from(code) % FONT_COLUMNS) as f32,
            (u32::from(code) / FONT_COLUMNS) as f32,
            1.0 / FONT_COLUMNS as f32,
        );
        self.push(
            font.number,
            [x, y, x + CHARACTER_SIZE, y + CHARACTER_SIZE],
            [
                column * step,
                row * step,
                (column + 1.0) * step,
                (row + 1.0) * step,
            ],
            PLAIN,
        );
    }

    /// Draws `text` in the font. Spaces are left clear.
    pub fn text(&mut self, x: f32, y: f32, text: &[u8]) {
        let mut at = x;
        for &code in text {
            if code != b' ' {
                self.character(at, y, code);
            }
            at += CHARACTER_SIZE;
        }
    }

    /// Draws `text` in the font's other colour.
    pub fn bright_text(&mut self, x: f32, y: f32, text: &[u8]) {
        let mut at = x;
        for &code in text {
            if code != b' ' {
                self.character(at, y, code | 128);
            }
            at += CHARACTER_SIZE;
        }
    }

    /// What's been drawn, in order.
    #[must_use]
    pub fn into_quads(self) -> Vec<Quad> {
        self.quads
    }
}
