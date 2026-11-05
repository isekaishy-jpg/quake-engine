//! The camera matrix.

use engine_core::math::{Vec3, view_axes};

/// `a` after `b`, for matrices stored column by column.
pub(crate) fn multiply(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0; 16];
    for column in 0..4 {
        for row in 0..4 {
            out[column * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[column * 4 + k]).sum();
        }
    }
    out
}

/// The nearest distance drawn.
const NEAR: f32 = 4.0;

/// The furthest distance drawn.
const FAR: f32 = 65536.0;

/// Screens are this wide for each unit of height when `fov` is exact.
const REFERENCE_ASPECT: f32 = 4.0 / 3.0;

/// The direction of a view with roll, as the forward, right and up axes.
pub(crate) fn rolled_axes(yaw: f32, pitch: f32, roll: f32) -> [Vec3; 3] {
    let [forward, right, up] = view_axes(yaw, pitch);
    let (roll_sin, roll_cos) = roll.to_radians().sin_cos();
    [
        forward,
        right * roll_cos - up * roll_sin,
        up * roll_cos + right * roll_sin,
    ]
}

/// A column-major matrix from world space to clip space, with y up and depth
/// from 0 to 1. That's the space WGSL uses: `naga` flips y for Vulkan when it
/// compiles the shader, so this mustn't flip it too.
///
/// `fov` is the horizontal angle in degrees on a 4:3 screen, and wider screens
/// see further to the sides. `angles` are the yaw, pitch and roll in degrees,
/// and positive pitch looks down.
#[must_use]
pub fn view_projection(origin: Vec3, angles: [f32; 3], fov: f32, aspect: f32) -> [f32; 16] {
    let [forward, right, up] = rolled_axes(angles[0], angles[1], angles[2]);
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

/// The inverse of a column-major 4 by 4 matrix, if it has one.
#[must_use]
pub(crate) fn inverse(m: &[f32; 16]) -> Option<[f32; 16]> {
    let wide = m.map(f64::from);
    let mut work = [[0.0_f64; 8]; 4];
    for row in 0..4 {
        for column in 0..4 {
            work[row][column] = wide[column * 4 + row];
        }
        work[row][4 + row] = 1.0;
    }
    for column in 0..4 {
        let pivot =
            (column..4).max_by(|a, b| work[*a][column].abs().total_cmp(&work[*b][column].abs()))?;
        if work[pivot][column].abs() < 1e-12 {
            return None;
        }
        work.swap(column, pivot);
        let scale = work[column][column];
        for value in &mut work[column] {
            *value /= scale;
        }
        for row in 0..4 {
            if row != column {
                let factor = work[row][column];
                let source = work[column];
                for (value, from) in work[row].iter_mut().zip(source) {
                    *value -= factor * from;
                }
            }
        }
    }
    let mut out = [0.0; 16];
    for row in 0..4 {
        for column in 0..4 {
            #[expect(clippy::cast_possible_truncation, reason = "shaders use f32")]
            let value = work[row][4 + column] as f32;
            out[column * 4 + row] = value;
        }
    }
    Some(out)
}
