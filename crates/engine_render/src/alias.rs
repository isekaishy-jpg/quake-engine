//! Alias models on the GPU.

use engine_core::math::Vec3;
use engine_map::AliasModel;

/// Bytes in one corner of one pose: a position, then a normal.
pub(crate) const POSE_CORNER_BYTES: usize = 8;

/// Bytes of the values a draw pushes.
pub(crate) const ALIAS_PUSH_SIZE: u32 = 112;

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

/// Each pose's corners, one after the other, with a byte position and normal.
#[must_use]
pub fn pose_bytes(model: &AliasModel) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(model.poses.len() * model.corners.len() * POSE_CORNER_BYTES);
    for pose in &model.poses {
        for corner in &model.corners {
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

/// Each corner's place on the skin.
#[must_use]
pub fn uv_bytes(model: &AliasModel) -> Vec<u8> {
    model
        .corners
        .iter()
        .flat_map(|corner| corner.uv)
        .flat_map(f32::to_le_bytes)
        .collect()
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

/// The values one draw pushes.
#[must_use]
pub fn push_bytes(draw: &EntityDraw, model: &AliasModel) -> Vec<u8> {
    let matrix = model_matrix(draw.origin, draw.angles);
    let (scale, offset) = (model.scale, model.offset);
    let in_front = if draw.in_front { 1.0 } else { 0.0 };
    let values = [
        scale.x,
        scale.y,
        scale.z,
        draw.blend, //
        offset.x,
        offset.y,
        offset.z,
        0.0, //
        draw.ambient,
        draw.shade,
        in_front,
        0.0,
    ];
    matrix
        .iter()
        .chain(&values)
        .flat_map(|value| value.to_le_bytes())
        .collect()
}
