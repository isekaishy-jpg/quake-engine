//! Working out a surface's normal and specular maps from its picture, for art
//! that came with neither (proposal 0099).
//!
//! A picture's brightness stands in for its height. The broad light and dark of
//! the picture isn't shape, so it's taken away first, which leaves the mortar,
//! cracks and rivets. Everything wraps, so what tiles still tiles.

use crate::error::CompileError;

/// How far the blur that finds the picture's broad tone reaches, in pixels.
const TONE_REACH: usize = 24;

/// How far the blur that settles fine grain reaches, in pixels.
const GRAIN_REACH: usize = 1;

/// How steep a typical slope comes out, at a strength of 1.
const TYPICAL_SLOPE: f32 = 0.55;

/// How much a pit or ridge moves its shine.
const SHINE_CONTRAST: f32 = 3.0;

/// How a surface's maps are worked out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeriveSettings {
    /// How strong its relief is, where 1 is usual.
    pub strength: f32,
    /// How much it shines, from 0 to 1.
    pub shine: f32,
}

/// What's worked out, in RGBA bytes the size of the picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivedMaps {
    /// Normals.
    pub normal: Vec<u8>,
    /// Specular colour, with gloss in alpha.
    pub specular: Vec<u8>,
}

/// One blur along rows, wrapping.
fn blur_rows(values: &[f32], width: usize, height: usize, reach: usize) -> Vec<f32> {
    let mut out = vec![0.0; values.len()];
    #[expect(clippy::cast_precision_loss, reason = "a short count")]
    let share = 1.0 / (reach * 2 + 1) as f32;
    for y in 0..height {
        let row = &values[y * width..(y + 1) * width];
        let mut total: f32 = (0..=reach * 2)
            .map(|index| row[(index + width - reach % width) % width])
            .sum();
        for x in 0..width {
            out[y * width + x] = total * share;
            total += row[(x + reach + 1) % width] - row[(x + width - reach % width) % width];
        }
    }
    out
}

/// `values` turned on their side.
fn flipped(values: &[f32], width: usize, height: usize) -> Vec<f32> {
    let mut out = vec![0.0; values.len()];
    for y in 0..height {
        for x in 0..width {
            out[x * height + y] = values[y * width + x];
        }
    }
    out
}

/// A blur both ways, wrapping.
fn blurred(values: &[f32], width: usize, height: usize, reach: usize) -> Vec<f32> {
    let across = blur_rows(values, width, height, reach);
    let turned = flipped(&across, width, height);
    let down = blur_rows(&turned, height, width, reach);
    flipped(&down, height, width)
}

/// A value from -1 to 1 as a byte.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to a byte"
)]
fn signed_byte(value: f32) -> u8 {
    ((value * 0.5 + 0.5) * 255.0).round().clamp(0.0, 255.0) as u8
}

/// A value from 0 to 1 as a byte.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to a byte"
)]
fn unsigned_byte(value: f32) -> u8 {
    (value * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Works out the maps of a picture of `width` by `height` in RGBA bytes.
#[must_use]
pub fn derive_maps(width: u32, height: u32, rgba: &[u8], settings: DeriveSettings) -> DerivedMaps {
    let (width, height) = (width as usize, height as usize);
    let (pixels, _) = rgba.as_chunks::<4>();
    let brightness: Vec<f32> = pixels
        .iter()
        .map(|[red, green, blue, _]| {
            (f32::from(*red) * 0.2126 + f32::from(*green) * 0.7152 + f32::from(*blue) * 0.0722)
                / 255.0
        })
        .collect();
    let tone = blurred(&brightness, width, height, TONE_REACH.min(width / 3));
    let relief: Vec<f32> = brightness.iter().zip(&tone).map(|(a, b)| a - b).collect();
    let shape = blurred(&relief, width, height, GRAIN_REACH);
    let at = |x: usize, y: usize| shape[(y % height) * width + x % width];
    let mut slopes = Vec::with_capacity(shape.len());
    for y in 0..height {
        for x in 0..width {
            let dx = at(x + 1, y) - at(x + width - 1, y);
            let dy = at(x, y + 1) - at(x, y + height - 1);
            slopes.push((dx, dy));
        }
    }
    // However faint a picture's relief, its usual slope comes out the same.
    let sum: f32 = slopes.iter().map(|(dx, dy)| dx * dx + dy * dy).sum();
    #[expect(clippy::cast_precision_loss, reason = "image sizes are small")]
    let typical = (sum / slopes.len().max(1) as f32).sqrt();
    let scale = if typical > 1e-5 {
        TYPICAL_SLOPE * settings.strength / typical
    } else {
        0.0
    };
    let mut normal = Vec::with_capacity(slopes.len() * 4);
    let mut specular = Vec::with_capacity(slopes.len() * 4);
    for ((dx, dy), height_here) in slopes.iter().zip(&shape) {
        let (nx, ny) = (-dx * scale, -dy * scale);
        let length = (nx * nx + ny * ny + 1.0).sqrt();
        normal.extend_from_slice(&[
            signed_byte(nx / length),
            signed_byte(ny / length),
            signed_byte(1.0 / length),
            255,
        ]);
        // What stands proud catches the light, and what's sunk doesn't.
        let shine = settings.shine * (0.5 + height_here * SHINE_CONTRAST).clamp(0.0, 1.0);
        let part = unsigned_byte(shine);
        specular.extend_from_slice(&[part, part, part, unsigned_byte(0.35 + shine * 0.65)]);
    }
    DerivedMaps { normal, specular }
}

/// A picture of `width` by `height` in RGBA bytes as a PNG.
///
/// # Errors
///
/// Returns an error if the picture can't be written.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, CompileError> {
    let bad = |error: &dyn std::fmt::Display| CompileError::Bad(error.to_string());
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| bad(&error))?;
    writer.write_image_data(rgba).map_err(|error| bad(&error))?;
    writer.finish().map_err(|error| bad(&error))?;
    Ok(out)
}
