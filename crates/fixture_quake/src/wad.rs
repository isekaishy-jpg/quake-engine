//! Flat pictures: `gfx.wad` and `.lmp` files, following `wad.c` and `draw.c`.

use std::collections::HashMap;

use engine_assets::FileSystem;
use engine_core::canvas::{FONT_PICTURE, Image};

use crate::texture::Palette;

/// The bytes in a WAD's header.
const HEADER_SIZE: usize = 12;

/// The bytes in an entry of a WAD's directory.
const ENTRY_SIZE: usize = 32;

/// The bytes in a lump's name.
const NAME_SIZE: usize = 16;

/// The palette index of a clear pixel.
const CLEAR: u8 = 255;

/// The size of the font in pixels each way, which has no header.
const FONT_SIZE: u32 = 128;

/// Reads a little-endian number at `at`.
fn number(bytes: &[u8], at: usize) -> Option<u32> {
    let field = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes(field.try_into().ok()?))
}

/// Colours `pixels`, with `clear` as the index that shows nothing.
fn image(width: u32, height: u32, pixels: &[u8], palette: &Palette, clear: u8) -> Image {
    let mut rgba = Vec::with_capacity(pixels.len() * 4);
    for &index in pixels {
        let [red, green, blue] = palette.color(index);
        let alpha = if index == clear { 0 } else { 255 };
        rgba.extend_from_slice(&[red, green, blue, alpha]);
    }
    Image {
        width,
        height,
        rgba,
    }
}

/// Reads a picture: its width and height, then a byte a pixel.
#[must_use]
pub fn parse_picture(bytes: &[u8], palette: &Palette) -> Option<Image> {
    let (width, height) = (number(bytes, 0)?, number(bytes, 4)?);
    let count = usize::try_from(width.checked_mul(height)?).ok()?;
    let pixels = bytes.get(8..8usize.checked_add(count)?)?;
    Some(image(width, height, pixels, palette, CLEAR))
}

/// `gfx.wad`: lumps by lower-case name.
#[derive(Clone, Debug, Default)]
pub struct Wad {
    /// Each lump's bytes.
    lumps: HashMap<String, Vec<u8>>,
}

impl Wad {
    /// Reads a WAD2 file.
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.get(..4)? != b"WAD2" {
            return None;
        }
        let count = usize::try_from(number(bytes, 4)?).ok()?;
        let directory = usize::try_from(number(bytes, 8)?).ok()?;
        if directory < HEADER_SIZE || count > bytes.len() / ENTRY_SIZE {
            return None;
        }
        let mut lumps = HashMap::with_capacity(count);
        for index in 0..count {
            let entry = bytes.get(directory + index * ENTRY_SIZE..)?;
            let start = usize::try_from(number(entry, 0)?).ok()?;
            let size = usize::try_from(number(entry, 8)?).ok()?;
            let name = entry.get(ENTRY_SIZE - NAME_SIZE..ENTRY_SIZE)?;
            let end = name.iter().position(|&byte| byte == 0).unwrap_or(NAME_SIZE);
            let name = String::from_utf8_lossy(&name[..end]).to_lowercase();
            lumps.insert(name, bytes.get(start..start.checked_add(size)?)?.to_vec());
        }
        Some(Self { lumps })
    }

    /// The picture called `name`. The font has no header, and shows nothing
    /// where its pixels are 0, as `Draw_Init` arranges.
    #[must_use]
    pub fn picture(&self, name: &str, palette: &Palette) -> Option<Image> {
        let lump = self.lumps.get(name)?;
        if name == FONT_PICTURE {
            let count = usize::try_from(FONT_SIZE * FONT_SIZE).ok()?;
            return Some(image(FONT_SIZE, FONT_SIZE, lump.get(..count)?, palette, 0));
        }
        parse_picture(lump, palette)
    }
}

/// The picture called `name`: a file if it ends in `.lmp`, otherwise a lump of
/// `wad`.
#[must_use]
pub fn load_picture(
    name: &str,
    filesystem: &FileSystem,
    wad: &Wad,
    palette: &Palette,
) -> Option<Image> {
    if name.ends_with(".lmp") {
        parse_picture(&filesystem.read(name).ok()?, palette)
    } else {
        wad.picture(name, palette)
    }
}
