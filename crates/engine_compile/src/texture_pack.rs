//! Pictures compiled for the GPU: mips made ahead, in block compression, so a
//! load only reads them (proposal 0106).

use std::collections::BTreeMap;
use std::path::Path;

use engine_format::{Block, Document, Value, from_binary, from_text, to_binary};
use engine_map::{MipLevel, PixelForm};
use rusty_dds::{Dds, DecodeContent, EncodeLayout};

use crate::error::CompileError;
use crate::light::in_parallel;
use crate::material::decode_png;

/// Where a picture's compiled form is kept.
#[must_use]
pub fn packed_path(picture: &str) -> String {
    format!("compiled/{picture}.etex")
}

/// What a picture holds, which says how it's averaged and compressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Colour, in sRGB.
    Colour,
    /// A measure of something, such as specular or a mask.
    Data,
    /// A normal map.
    Normal,
}

/// A number from a colour byte, in linear light.
fn linear(byte: u8) -> f32 {
    let value = f32::from(byte) / 255.0;
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// A colour byte from a number in linear light.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to a byte"
)]
fn colour_byte(value: f32) -> u8 {
    let encoded = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round().clamp(0.0, 255.0) as u8
}

/// A byte from a number from 0 to 1.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to a byte"
)]
fn plain_byte(value: f32) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

/// One level of a picture, as numbers, with colour in linear light so that
/// averaging it is right.
struct Level {
    /// Pixels across.
    width: usize,
    /// Pixels down.
    height: usize,
    /// Four numbers a pixel.
    values: Vec<f32>,
}

impl Level {
    /// The first level, from `rgba`.
    fn from_bytes(width: u32, height: u32, rgba: &[u8], role: Role) -> Self {
        let values = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|[r, g, b, a]| match role {
                Role::Colour => [linear(*r), linear(*g), linear(*b), f32::from(*a) / 255.0],
                Role::Data | Role::Normal => [*r, *g, *b, *a].map(|byte| f32::from(byte) / 255.0),
            })
            .collect();
        Self {
            width: width as usize,
            height: height as usize,
            values,
        }
    }

    /// The next level down. Pictures repeat, so an odd edge wraps. A normal is
    /// made a unit again, as an average of them is shorter.
    fn halved(&self, role: Role) -> Self {
        let (width, height) = ((self.width / 2).max(1), (self.height / 2).max(1));
        let mut values = Vec::with_capacity(width * height * 4);
        for y in 0..height {
            for x in 0..width {
                let mut pixel = [0.0_f32; 4];
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let from_x = (x * 2 + dx) % self.width;
                    let from_y = (y * 2 + dy) % self.height;
                    let at = (from_y * self.width + from_x) * 4;
                    for (sum, value) in pixel.iter_mut().zip(&self.values[at..at + 4]) {
                        *sum += value * 0.25;
                    }
                }
                if role == Role::Normal {
                    let unit = [pixel[0], pixel[1], pixel[2]].map(|part| part * 2.0 - 1.0);
                    let length = unit.iter().map(|part| part * part).sum::<f32>().sqrt();
                    let scale = 1.0 / length.max(1e-4);
                    for (out, part) in pixel.iter_mut().zip(unit) {
                        *out = part * scale * 0.5 + 0.5;
                    }
                }
                values.extend(pixel);
            }
        }
        Self {
            width,
            height,
            values,
        }
    }

    /// The level as bytes.
    fn bytes(&self, role: Role) -> Vec<u8> {
        let (pixels, _) = self.values.as_chunks::<4>();
        pixels
            .iter()
            .flat_map(|[r, g, b, a]| match role {
                Role::Colour => [
                    colour_byte(*r),
                    colour_byte(*g),
                    colour_byte(*b),
                    plain_byte(*a),
                ],
                Role::Data | Role::Normal => [*r, *g, *b, *a].map(plain_byte),
            })
            .collect()
    }
}

/// The compression for a role.
const fn form_of(role: Role) -> PixelForm {
    match role {
        Role::Colour => PixelForm::Bc7Srgb,
        Role::Data => PixelForm::Bc7,
        Role::Normal => PixelForm::Bc5,
    }
}

/// A picture's form in a file.
const fn form_name(form: PixelForm) -> &'static str {
    match form {
        PixelForm::Rgba => "rgba",
        PixelForm::Bc7Srgb => "bc7_srgb",
        PixelForm::Bc7 => "bc7",
        PixelForm::Bc5 => "bc5",
    }
}

/// A picture's form, from its name in a file.
fn form_named(name: &str) -> Option<PixelForm> {
    [
        PixelForm::Rgba,
        PixelForm::Bc7Srgb,
        PixelForm::Bc7,
        PixelForm::Bc5,
    ]
    .into_iter()
    .find(|form| form_name(*form) == name)
}

/// A picture as a compiled file, which says the `hash` of its source.
///
/// # Errors
///
/// Returns an error if it can't be compressed.
pub fn pack_picture(
    width: u32,
    height: u32,
    rgba: &[u8],
    role: Role,
    hash: u64,
) -> Result<Vec<u8>, CompileError> {
    let form = form_of(role);
    let content = if role == Role::Normal {
        DecodeContent::Bc5UNorm
    } else {
        DecodeContent::Bc7
    };
    let mut document = Document::new("texture");
    let mut source = Block::new("source");
    source.set("hash", Value::Text(format!("{hash:016x}")));
    source.set("form", Value::Text(form_name(form).to_owned()));
    document.blocks.push(source);
    let mut level = Level::from_bytes(width, height, rgba, role);
    loop {
        let (across, down) = (
            u32::try_from(level.width).unwrap_or(1),
            u32::try_from(level.height).unwrap_or(1),
        );
        let layout = EncodeLayout::flat_2d(content, across, down);
        let bad = |error| CompileError::Bad(format!("compressing a picture: {error:?}"));
        let encoded = Dds::encode_from_rgba8(&level.bytes(role), layout).map_err(bad)?;
        let mut block = Block::new("level");
        block.set("width", Value::Number(f64::from(across)));
        block.set("height", Value::Number(f64::from(down)));
        let blocks = encoded.get_data(0).map_err(bad)?;
        block.set("blocks", Value::Bytes(blocks.to_vec()));
        document.blocks.push(block);
        if level.width == 1 && level.height == 1 {
            break;
        }
        level = level.halved(role);
    }
    Ok(to_binary(&document))
}

/// The mips in a compiled picture, and the hash of its source, if it reads.
#[must_use]
pub fn read_packed(bytes: &[u8]) -> Option<(u64, Vec<MipLevel>)> {
    let document = from_binary(bytes).ok()?;
    if document.kind != "texture" {
        return None;
    }
    let source = document.blocks_of("source").next()?;
    let hash = u64::from_str_radix(source.get("hash")?.text()?, 16).ok()?;
    let form = form_named(source.get("form")?.text()?)?;
    let mut mips = Vec::new();
    for level in document.blocks_of("level") {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "picture sizes are small"
        )]
        let size = |key: &str| {
            level
                .get(key)
                .and_then(Value::number)
                .map(|side| side as u32)
        };
        mips.push(MipLevel {
            width: size("width")?,
            height: size("height")?,
            rgba: level.get("blocks")?.bytes()?.to_vec(),
            form,
        });
    }
    (!mips.is_empty()).then_some((hash, mips))
}

/// Each picture a material uses, and what it holds.
#[must_use]
pub fn picture_roles(document: &Document) -> Vec<(String, Role)> {
    let role_of = |key: &str| match key {
        "normal" => Role::Normal,
        "specular" | "height" | "flow" | "mask" | "detail" | "shader_image_a"
        | "shader_image_b" => Role::Data,
        _ => Role::Colour,
    };
    let mut found = Vec::new();
    for block in &document.blocks {
        for (key, value) in &block.fields {
            if let Some(path) = value.text().filter(|path| path.ends_with(".png")) {
                found.push((path.to_owned(), role_of(key)));
            }
        }
        for inner in &block.children {
            for (key, value) in &inner.fields {
                if let Some(path) = value.text().filter(|path| path.ends_with(".png")) {
                    found.push((path.to_owned(), role_of(key)));
                }
            }
        }
    }
    found
}

/// A hash of a picture's file, so its compiled form knows if it's behind.
#[must_use]
pub fn bytes_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for chunk in bytes.chunks(8) {
        let mut word = [0; 8];
        word[..chunk.len()].copy_from_slice(chunk);
        hash = (hash ^ u64::from_le_bytes(word)).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

/// Compiles a picture, unless what's there is up to date. Returns whether it did.
fn pack_one(root: &Path, picture: &str, role: Role) -> Result<bool, CompileError> {
    let bad = |error: std::io::Error| CompileError::Bad(format!("{picture}: {error}"));
    let bytes = std::fs::read(root.join(picture)).map_err(bad)?;
    let hash = bytes_hash(&bytes);
    let out = root.join(packed_path(picture));
    let kept = std::fs::read(&out).ok();
    if kept.is_some_and(|kept| read_packed(&kept).is_some_and(|(known, _)| known == hash)) {
        return Ok(false);
    }
    let (width, height, rgba) = decode_png(&bytes)?;
    let packed = pack_picture(width, height, &rgba, role, hash)?;
    if let Some(folder) = out.parent() {
        std::fs::create_dir_all(folder).map_err(bad)?;
    }
    std::fs::write(&out, packed).map_err(bad)?;
    Ok(true)
}

/// Compiles every picture the materials under `root` use. Returns how many it
/// compiled, and how many there are. A picture with two jobs, such as colour in
/// one material and a measure in another, stays as it is, since the GPU would
/// read it wrongly in one of them.
///
/// # Errors
///
/// Returns an error if a material or a picture can't be read.
pub fn pack_textures(root: &Path) -> Result<(usize, usize), CompileError> {
    let bad = |error: std::io::Error| CompileError::Bad(format!("materials: {error}"));
    let mut pictures = BTreeMap::new();
    for entry in std::fs::read_dir(root.join("materials")).map_err(bad)? {
        let path = entry.map_err(bad)?.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "emat")
        {
            let text = std::fs::read_to_string(&path).map_err(bad)?;
            for (picture, role) in picture_roles(&from_text(&text)?) {
                if root.join(&picture).exists() {
                    let known = pictures.entry(picture).or_insert(Some(role));
                    if *known != Some(role) {
                        *known = None;
                    }
                }
            }
        }
    }
    let mut wanted = Vec::with_capacity(pictures.len());
    for (picture, role) in pictures {
        if let Some(role) = role {
            wanted.push((picture, role));
        } else if let Err(error) = std::fs::remove_file(root.join(packed_path(&picture)))
            && error.kind() != std::io::ErrorKind::NotFound
        {
            return Err(bad(error));
        }
    }
    let done = in_parallel(&wanted, |(picture, role)| pack_one(root, picture, *role));
    let mut compiled = 0;
    for result in done {
        compiled += usize::from(result?);
    }
    Ok((compiled, wanted.len()))
}
