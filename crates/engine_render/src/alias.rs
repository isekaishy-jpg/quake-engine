//! The fixture's models, as what the mesh path draws (proposal 0142).

use std::collections::HashMap;

use engine_core::math::Vec3;
use engine_map::{AliasCorner, AliasModel, Rows};

use crate::instances::{LOOK_BYTES, PLACE_BYTES};

/// Bytes in one corner of one pose: a position, then a normal.
pub(crate) const POSE_CORNER_BYTES: usize = 8;

/// A model to draw this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EntityDraw {
    /// The number [`crate::Renderer::add_model`] gave it.
    pub model: u32,
    /// The pose to blend from.
    pub pose: u32,
    /// The pose to blend to.
    pub next_pose: u32,
    /// How far between them, from 0 to 1.
    pub blend: f32,
    /// Which skin.
    pub skin: u32,
    /// Where it is.
    pub origin: Vec3,
    /// Its pitch, yaw and roll in degrees, as a game gives them.
    pub angles: Vec3,
    /// Light from all round, where 1 leaves its skin as it is.
    pub ambient: f32,
    /// Light from one side, on the same scale.
    pub shade: f32,
    /// Whether it stays in front of everything, as the player's weapon does.
    pub in_front: bool,
}

/// A model's corners, shared where triangles share them, as a mesh's are.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeldedCorners {
    /// Each one: a vertex, and where it is on the skin.
    pub corners: Vec<AliasCorner>,
    /// Three to a triangle, wound as a mesh's are.
    pub indices: Vec<u32>,
}

/// Shares `model`'s corners. It keeps them three to a triangle, so most are there several times,
/// and each one kept here is one that's moved every frame.
#[must_use]
pub fn weld_corners(model: &AliasModel) -> WeldedCorners {
    let mut known = HashMap::new();
    let mut welded = WeldedCorners::default();
    for triangle in model.corners.as_chunks::<3>().0 {
        // Quake's triangles wind the other way from a mesh's.
        for corner in [triangle[0], triangle[2], triangle[1]] {
            let key = (corner.vertex, corner.uv.map(f32::to_bits));
            let index = *known.entry(key).or_insert_with(|| {
                welded.corners.push(corner);
                u32::try_from(welded.corners.len() - 1).unwrap_or(0)
            });
            welded.indices.push(index);
        }
    }
    welded
}

/// Each pose's corners, one after the other, with a byte position and normal.
#[must_use]
pub(crate) fn pose_bytes(model: &AliasModel, corners: &[AliasCorner]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(model.poses.len() * corners.len() * POSE_CORNER_BYTES);
    for pose in &model.poses {
        for corner in corners {
            let vertex = usize::try_from(corner.vertex)
                .ok()
                .and_then(|index| pose.get(index))
                .copied()
                .unwrap_or_default();
            let [x, y, z] = vertex.position;
            let [nx, ny, nz] = vertex.normal.map(i8::to_ne_bytes).map(|[byte]| byte);
            bytes.extend_from_slice(&[x, y, z, 0, nx, ny, nz, 0]);
        }
    }
    bytes
}

/// `corners`' places on the skin, as a mesh's corners keep them, with a colour that does nothing.
#[must_use]
pub(crate) fn look_bytes(corners: &[AliasCorner]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(corners.len() * LOOK_BYTES);
    for corner in corners {
        bytes.extend(corner.uv.into_iter().flat_map(f32::to_le_bytes));
        bytes.extend_from_slice(&[u8::MAX; 4]);
    }
    bytes
}

/// `corners` as a mesh's are, in `model`'s first pose, for when there's no room to move them.
#[must_use]
pub(crate) fn rest_bytes(model: &AliasModel, corners: &[AliasCorner]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(corners.len() * PLACE_BYTES);
    let first = model.poses.first();
    for corner in corners {
        let vertex = first
            .and_then(|pose| pose.get(usize::try_from(corner.vertex).ok()?))
            .copied()
            .unwrap_or_default();
        let [x, y, z] = vertex.position.map(f32::from);
        let (scale, offset) = (model.scale, model.offset);
        let place = [
            x * scale.x + offset.x,
            y * scale.y + offset.y,
            z * scale.z + offset.z,
        ];
        for number in place {
            bytes.extend_from_slice(&number.to_le_bytes());
        }
        bytes.extend(vertex.normal.map(i8::to_le_bytes).into_iter().flatten());
        bytes.push(127);
    }
    bytes
}

/// The matrix that places a model, column by column, from `R_AliasSetUpTransform`.
/// A model's pitch runs the other way from a view's.
#[must_use]
pub fn model_matrix(origin: Vec3, angles: Vec3) -> [f32; 16] {
    let (pitch_sin, pitch_cos) = (-angles.x).to_radians().sin_cos();
    let (yaw_sin, yaw_cos) = angles.y.to_radians().sin_cos();
    let (roll_sin, roll_cos) = angles.z.to_radians().sin_cos();
    let forward = [pitch_cos * yaw_cos, pitch_cos * yaw_sin, -pitch_sin];
    let left = [
        roll_sin * pitch_sin * yaw_cos - roll_cos * yaw_sin,
        roll_sin * pitch_sin * yaw_sin + roll_cos * yaw_cos,
        roll_sin * pitch_cos,
    ];
    let up = [
        roll_cos * pitch_sin * yaw_cos + roll_sin * yaw_sin,
        roll_cos * pitch_sin * yaw_sin - roll_sin * yaw_cos,
        roll_cos * pitch_cos,
    ];
    [
        forward[0], forward[1], forward[2], 0.0, //
        left[0], left[1], left[2], 0.0, //
        up[0], up[1], up[2], 0.0, //
        origin.x, origin.y, origin.z, 1.0,
    ]
}

/// `matrix`, which is by columns, as the rows an instance keeps.
#[must_use]
pub(crate) const fn rows_of_matrix(matrix: &[f32; 16]) -> Rows {
    let m = matrix;
    [
        [m[0], m[4], m[8], m[12]],
        [m[1], m[5], m[9], m[13]],
        [m[2], m[6], m[10], m[14]],
    ]
}
