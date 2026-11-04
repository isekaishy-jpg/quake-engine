//! Quake's sprite files, from `spritegn.h` and `Mod_LoadSpriteModel`.

use engine_map::{Sprite, SpriteFacing, SpriteFrame, SpriteImage, TextureKind};

use crate::texture::Palette;

/// The file's first four bytes.
const MAGIC: &[u8; 4] = b"IDSP";

/// The only version there is.
const VERSION: i32 = 1;

/// Bytes before the first frame.
const HEADER_BYTES: usize = 36;

/// The most frames or pictures a sensible file has.
const MAX_FRAMES: usize = 4096;

/// A place in a file, moving forward as it's read.
struct Reader<'a> {
    /// What's left.
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    /// The next `count` bytes.
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let (taken, rest) = self.bytes.split_at_checked(count)?;
        self.bytes = rest;
        Some(taken)
    }

    /// The next whole number.
    fn int(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    /// The next real number.
    fn float(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    /// The next count, up to [`MAX_FRAMES`].
    fn count(&mut self) -> Option<usize> {
        usize::try_from(self.int()?)
            .ok()
            .filter(|count| *count <= MAX_FRAMES)
    }

    /// The next picture.
    fn image(&mut self, palette: &Palette) -> Option<SpriteImage> {
        let left = self.int()?;
        let top = self.int()?;
        let width = u32::try_from(self.int()?).ok()?;
        let height = u32::try_from(self.int()?).ok()?;
        let size = usize::try_from(width.checked_mul(height)?).ok()?;
        let mip = palette.expand(self.take(size)?, width, height, TextureKind::Cutout);
        Some(SpriteImage {
            left,
            top,
            width,
            height,
            rgba: mip.rgba,
        })
    }
}

/// How a file's type number turns a sprite.
fn facing(number: i32) -> Option<SpriteFacing> {
    match number {
        0 => Some(SpriteFacing::ViewUpright),
        1 => Some(SpriteFacing::FacingUpright),
        2 => Some(SpriteFacing::View),
        3 => Some(SpriteFacing::Oriented),
        4 => Some(SpriteFacing::ViewRolled),
        _ => None,
    }
}

/// Reads a sprite, or `None` if `bytes` isn't one.
#[must_use]
pub fn parse_spr(bytes: &[u8], palette: &Palette) -> Option<Sprite> {
    let mut reader = Reader { bytes };
    let header = reader.take(HEADER_BYTES)?;
    let mut fields = Reader { bytes: header };
    (fields.take(4)? == MAGIC).then_some(())?;
    (fields.int()? == VERSION).then_some(())?;
    let facing = facing(fields.int()?)?;
    let _radius = fields.float()?;
    let _width = fields.int()?;
    let _height = fields.int()?;
    let frame_count = fields.count()?;
    let mut sprite = Sprite {
        facing,
        ..Sprite::default()
    };
    for _ in 0..frame_count {
        let first_image = u32::try_from(sprite.images.len()).ok()?;
        if reader.int()? == 0 {
            sprite.images.push(reader.image(palette)?);
            sprite.frames.push(SpriteFrame {
                first_image,
                image_count: 1,
                image_ends: Vec::new(),
            });
        } else {
            let count = reader.count().filter(|count| *count > 0)?;
            let mut image_ends = Vec::with_capacity(count);
            for _ in 0..count {
                image_ends.push(reader.float()?);
            }
            for _ in 0..count {
                sprite.images.push(reader.image(palette)?);
            }
            sprite.frames.push(SpriteFrame {
                first_image,
                image_count: u32::try_from(count).ok()?,
                image_ends,
            });
        }
    }
    (!sprite.frames.is_empty()).then_some(sprite)
}
