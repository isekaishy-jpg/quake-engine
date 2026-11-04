//! The camera matrix.

use engine_core::math::Vec3;

/// The nearest distance drawn.
const NEAR: f32 = 4.0;

/// The furthest distance drawn.
const FAR: f32 = 65536.0;

/// Screens are this wide for each unit of height when `fov` is exact.
const REFERENCE_ASPECT: f32 = 4.0 / 3.0;

/// The direction of a view, as the forward, right and up axes.
fn view_axes(yaw: f32, pitch: f32) -> [Vec3; 3] {
    let (yaw_sin, yaw_cos) = yaw.to_radians().sin_cos();
    let (pitch_sin, pitch_cos) = pitch.to_radians().sin_cos();
    let forward = Vec3::new(yaw_cos * pitch_cos, yaw_sin * pitch_cos, -pitch_sin);
    let right = Vec3::new(yaw_sin, -yaw_cos, 0.0);
    let up = Vec3::new(yaw_cos * pitch_sin, yaw_sin * pitch_sin, pitch_cos);
    [forward, right, up]
}

/// A column-major matrix from world space to clip space, with y up and depth
/// from 0 to 1. That's the space WGSL uses: `naga` flips y for Vulkan when it
/// compiles the shader, so this mustn't flip it too.
///
/// `fov` is the horizontal angle in degrees on a 4:3 screen, and wider screens
/// see further to the sides. Positive pitch looks down.
#[must_use]
pub fn view_projection(origin: Vec3, yaw: f32, pitch: f32, fov: f32, aspect: f32) -> [f32; 16] {
    let [forward, right, up] = view_axes(yaw, pitch);
    let scale_y = REFERENCE_ASPECT / (fov.to_radians() * 0.5).tan();
    let scale_x = scale_y / aspect;
    let depth = FAR / (FAR - NEAR);
    let offset = -NEAR * depth;
    let row = |axis: Vec3, scale: f32, add: f32| {
        [
            axis.x * scale,
            axis.y * scale,
            axis.z * scale,
            -axis.dot(origin) * scale + add,
        ]
    };
    let x = row(right, scale_x, 0.0);
    let y = row(up, scale_y, 0.0);
    let z = row(forward, depth, offset);
    let w = row(forward, 1.0, 0.0);
    [
        x[0], y[0], z[0], w[0], x[1], y[1], z[1], w[1], x[2], y[2], z[2], w[2], x[3], y[3], z[3],
        w[3],
    ]
}
