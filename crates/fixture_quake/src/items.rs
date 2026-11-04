//! Health and ammo boxes, which are small BSP maps rather than alias models.
//!
//! Each texture becomes a model of its own with a single pose, so they draw
//! like everything else.

use engine_core::math::Vec3;
use engine_map::{AliasCorner, AliasFrame, AliasModel, AliasVertex, Map, TextureKind};

use crate::bsp::import_map;
use crate::texture::Palette;

/// The most a byte position can be.
const POSITION_STEPS: f32 = 255.0;

/// The points of face `index` in a map.
fn face_points(map: &Map, index: usize) -> Option<&[Vec3]> {
    let face = map.faces.get(index)?;
    let first = usize::try_from(face.first_point).ok()?;
    let count = usize::try_from(face.point_count).ok()?;
    map.points.get(first..first + count)
}

/// A normal as signed bytes.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a normal's parts are from -1 to 1, so this fits"
)]
fn normal_bytes(normal: Vec3) -> [i8; 3] {
    [normal.x, normal.y, normal.z].map(|part| (part * 127.0).round() as i8)
}

/// A position as bytes, within a box of `mins` with `scale` per step.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a point is inside its box, so each part is from 0 to 255"
)]
fn position_bytes(point: Vec3, mins: Vec3, scale: Vec3) -> [u8; 3] {
    let step = |value: f32, low: f32, size: f32| {
        if size > 0.0 {
            ((value - low) / size).round() as u8
        } else {
            0
        }
    };
    [
        step(point.x, mins.x, scale.x),
        step(point.y, mins.y, scale.y),
        step(point.z, mins.z, scale.z),
    ]
}

/// The model of everything in `map` that uses texture `number`, if anything.
#[expect(
    clippy::cast_precision_loss,
    reason = "textures are far smaller than f32 can count"
)]
fn model_of_texture(map: &Map, number: u32, mins: Vec3, scale: Vec3) -> Option<AliasModel> {
    let texture = map.textures.get(usize::try_from(number).ok()?)?;
    if texture.kind == TextureKind::Hidden {
        return None;
    }
    let mip = texture.mips.first()?;
    let (width, height) = (mip.width as f32, mip.height as f32);
    let mut corners = Vec::new();
    let mut vertices = Vec::new();
    for (index, face) in map.faces.iter().enumerate() {
        if face.texture != number {
            continue;
        }
        let Some(points) = face_points(map, index) else {
            continue;
        };
        let [first, second, third, ..] = points else {
            continue;
        };
        // Faces wind clockwise from the front.
        let normal = (*third - *first).cross(*second - *first).normalized();
        for fan in 1..points.len().saturating_sub(1) {
            for point in [points[0], points[fan], points[fan + 1]] {
                corners.push(AliasCorner {
                    vertex: u32::try_from(vertices.len()).ok()?,
                    uv: [
                        face.axes[0].project(point) / width,
                        face.axes[1].project(point) / height,
                    ],
                });
                vertices.push(AliasVertex {
                    position: position_bytes(point, mins, scale),
                    normal: normal_bytes(normal),
                });
            }
        }
    }
    if corners.is_empty() {
        return None;
    }
    Some(AliasModel {
        scale,
        offset: mins,
        skin_width: mip.width,
        skin_height: mip.height,
        skins: vec![mip.rgba.clone()],
        corners,
        poses: vec![vertices],
        frames: vec![AliasFrame {
            first_pose: 0,
            pose_count: 1,
            pose_ends: Vec::new(),
        }],
        spins: false,
        flags: 0,
    })
}

/// Reads a BSP file as models to draw, one for each texture it uses.
#[must_use]
pub fn parse_item_bsp(bytes: &[u8], palette: &Palette) -> Option<Vec<AliasModel>> {
    let map = import_map(bytes, None, palette).ok()?;
    let (first, rest) = map.points.split_first()?;
    let (mins, maxs) = rest.iter().fold((*first, *first), |(low, high), point| {
        (
            Vec3::new(low.x.min(point.x), low.y.min(point.y), low.z.min(point.z)),
            Vec3::new(
                high.x.max(point.x),
                high.y.max(point.y),
                high.z.max(point.z),
            ),
        )
    });
    let size = maxs - mins;
    let scale = Vec3::new(
        size.x / POSITION_STEPS,
        size.y / POSITION_STEPS,
        size.z / POSITION_STEPS,
    );
    let textures = u32::try_from(map.textures.len()).ok()?;
    let models: Vec<AliasModel> = (0..textures)
        .filter_map(|number| model_of_texture(&map, number, mins, scale))
        .collect();
    (!models.is_empty()).then_some(models)
}
