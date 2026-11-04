//! Quake's alias models, following `Mod_LoadAliasModel` in `model.c`.

use engine_core::math::Vec3;
use engine_map::{AliasCorner, AliasFrame, AliasModel, AliasVertex, TextureKind};

use crate::anorms::NORMALS;
use crate::texture::Palette;

/// `IDPO`.
const MAGIC: &[u8; 4] = b"IDPO";

/// `ALIAS_VERSION`.
const VERSION: i32 = 6;

/// The header's size.
const HEADER_BYTES: usize = 84;

/// The flag that makes a model spin.
const SPINS: u32 = 8;

/// The most of anything a sane file has, against hostile counts.
const MAX_COUNT: usize = 1 << 20;

/// Reads from the front of a file.
struct Reader<'a> {
    /// What's left.
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    /// Takes `count` bytes.
    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let (taken, rest) = self.bytes.split_at_checked(count)?;
        self.bytes = rest;
        Some(taken)
    }

    /// A number.
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    /// A count, which has to be sane.
    fn count(&mut self) -> Option<usize> {
        usize::try_from(self.i32()?)
            .ok()
            .filter(|count| *count <= MAX_COUNT)
    }

    /// A float.
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_le_bytes(self.take(4)?.try_into().ok()?))
    }

    /// A vector.
    fn vec3(&mut self) -> Option<Vec3> {
        Some(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }
}

/// One part of a normal, from -1 to 1, as a signed byte.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a normal's parts are from -1 to 1, so this fits"
)]
fn signed_byte(part: f32) -> i8 {
    (part * 127.0).round() as i8
}

/// Reads one pose of `vertices` vertices, after its bounds and name.
fn read_pose(reader: &mut Reader<'_>, vertices: usize) -> Option<Vec<AliasVertex>> {
    reader.take(8 + 16)?;
    let bytes = reader.take(vertices.checked_mul(4)?)?;
    let (vertices, _) = bytes.as_chunks::<4>();
    vertices
        .iter()
        .map(|&[x, y, z, normal]| {
            let normal = NORMALS.get(usize::from(normal))?;
            Some(AliasVertex {
                position: [x, y, z],
                normal: normal.map(signed_byte),
            })
        })
        .collect()
}

/// Reads the frames, into poses and runs of them.
fn read_frames(
    reader: &mut Reader<'_>,
    count: usize,
    vertices: usize,
) -> Option<(Vec<Vec<AliasVertex>>, Vec<AliasFrame>)> {
    let mut poses = Vec::new();
    let mut frames = Vec::with_capacity(count);
    for _ in 0..count {
        let first_pose = u32::try_from(poses.len()).ok()?;
        if reader.i32()? == 0 {
            poses.push(read_pose(reader, vertices)?);
            frames.push(AliasFrame {
                first_pose,
                pose_count: 1,
                pose_ends: Vec::new(),
            });
            continue;
        }
        let run = reader.count().filter(|run| *run > 0)?;
        reader.take(8)?;
        let pose_ends = (0..run)
            .map(|_| reader.f32())
            .collect::<Option<Vec<f32>>>()?;
        for _ in 0..run {
            poses.push(read_pose(reader, vertices)?);
        }
        frames.push(AliasFrame {
            first_pose,
            pose_count: u32::try_from(run).ok()?,
            pose_ends,
        });
    }
    Some((poses, frames))
}

/// The palette rows a skin's shirt and pants are painted in, from `TOP_RANGE`
/// and `BOTTOM_RANGE`.
const SHIRT_ROW: u8 = 16;

/// The row of the pants.
const PANTS_ROW: u8 = 96;

/// What each palette index becomes for `colors`, from `CL_NewTranslation`. The
/// artists made the rows from 128 up run backwards.
fn translation(colors: u8) -> [u8; 256] {
    let mut table = [0; 256];
    for (index, entry) in (0u8..=255).zip(&mut table) {
        *entry = index;
    }
    for (first, row) in [(SHIRT_ROW, colors >> 4), (PANTS_ROW, colors & 15)] {
        let start = row * 16;
        for step in 0..16u8 {
            table[usize::from(first + step)] = if start < 128 {
                start + step
            } else {
                start + 15 - step
            };
        }
    }
    table
}

/// Reads the skins as RGBA, taking the first of any group, with each index
/// swapped through `table`.
fn read_skins(
    reader: &mut Reader<'_>,
    count: usize,
    size: usize,
    (width, height): (u32, u32),
    palette: &Palette,
    table: &[u8; 256],
) -> Option<Vec<Vec<u8>>> {
    let expand = |indices: &[u8]| {
        let swapped: Vec<u8> = indices
            .iter()
            .map(|&index| table[usize::from(index)])
            .collect();
        palette
            .expand(&swapped, width, height, TextureKind::Solid)
            .rgba
    };
    let mut skins = Vec::with_capacity(count);
    for _ in 0..count {
        if reader.i32()? == 0 {
            skins.push(expand(reader.take(size)?));
            continue;
        }
        let run = reader.count().filter(|run| *run > 0)?;
        reader.take(run.checked_mul(4)?)?;
        skins.push(expand(reader.take(size)?));
        reader.take((run - 1).checked_mul(size)?)?;
    }
    Some(skins)
}

/// Reads a version 6 MDL, or `None` for anything else. Palette indices from
/// 224 up are fullbright, as on the world.
#[must_use]
pub fn parse_mdl(bytes: &[u8], palette: &Palette) -> Option<AliasModel> {
    parse_colored_mdl(bytes, palette, 0)
}

/// Reads an MDL with its shirt and pants in `colors`, as the high and low four
/// bits. Colours of 0 and 1 paint them as they were drawn.
#[must_use]
pub fn parse_colored_mdl(bytes: &[u8], palette: &Palette, colors: u8) -> Option<AliasModel> {
    let mut reader = Reader { bytes };
    if reader.take(4)? != MAGIC || reader.i32()? != VERSION {
        return None;
    }
    let scale = reader.vec3()?;
    let offset = reader.vec3()?;
    reader.take(4 + 12)?;
    let skin_count = reader.count()?;
    let width = u32::try_from(reader.count()?).ok()?;
    let height = u32::try_from(reader.count()?).ok()?;
    let vertices = reader.count()?;
    let triangles = reader.count()?;
    let frame_count = reader.count()?;
    reader.take(4)?;
    let flags = u32::try_from(reader.i32()?).ok()?;
    reader.take(4)?;
    debug_assert_eq!(bytes.len() - reader.bytes.len(), HEADER_BYTES);
    if width == 0 || height == 0 || frame_count == 0 {
        return None;
    }
    let size = usize::try_from(width.checked_mul(height)?).ok()?;
    let table = translation(if colors == 0 { 0x16 } else { colors });
    let skins = read_skins(
        &mut reader,
        skin_count,
        size,
        (width, height),
        palette,
        &table,
    )?;

    let points = reader.take(vertices.checked_mul(12)?)?;
    let point = |index: usize| -> Option<(bool, f32, f32)> {
        let entry = points.get(index * 12..index * 12 + 12)?;
        let field = |at: usize| {
            i32::from_le_bytes([entry[at], entry[at + 1], entry[at + 2], entry[at + 3]])
        };
        Some((field(0) != 0, field(4) as f32, field(8) as f32))
    };
    let mut corners = Vec::with_capacity(triangles * 3);
    for _ in 0..triangles {
        let front = reader.i32()? != 0;
        for _ in 0..3 {
            let vertex = reader.count().filter(|vertex| *vertex < vertices)?;
            let (seam, s, t) = point(vertex)?;
            let s = if seam && !front {
                s + width as f32 / 2.0
            } else {
                s
            };
            corners.push(AliasCorner {
                vertex: u32::try_from(vertex).ok()?,
                uv: [(s + 0.5) / width as f32, (t + 0.5) / height as f32],
            });
        }
    }
    let (poses, frames) = read_frames(&mut reader, frame_count, vertices)?;
    Some(AliasModel {
        scale,
        offset,
        skin_width: width,
        skin_height: height,
        skins,
        corners,
        poses,
        frames,
        spins: flags & SPINS != 0,
        flags,
    })
}
