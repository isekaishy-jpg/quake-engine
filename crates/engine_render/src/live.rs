//! Moving lights: clusters that say which reach each part of the view, and the
//! uniforms the shaders read them from (proposal 0084).

use engine_core::math::Vec3;

use crate::backend::{SUN_REACHES, SunShadow};
use crate::matrix::rolled_axes;
use crate::renderer::SunDraw;
use crate::renderer::{CapsuleDraw, View};

/// The most moving lights in a frame, one for each of a cluster's first bits.
pub const MAX_LIVE_LIGHTS: usize = 48;

/// The most capsules in a frame, one for each of a cluster's last bits. A capsule
/// only shades what's close behind it, so few pixels should look at any of them.
pub(crate) const MAX_CAPSULES: usize = 16;

/// How far behind a capsule its shade runs.
const CAPSULE_REACH: f32 = 256.0;

/// The most lights with shadows, at six atlas tiles each.
pub const MAX_SHADOW_LIGHTS: usize = 10;

/// Clusters across the view.
pub const CLUSTERS_X: usize = 16;

/// Clusters down the view.
pub const CLUSTERS_Y: usize = 9;

/// Clusters into the view.
pub const CLUSTERS_Z: usize = 16;

/// How near the first slice starts.
const SLICE_NEAR: f32 = 16.0;

/// How far the last slice ends.
const SLICE_FAR: f32 = 16384.0;

/// Bytes of a light in the uniforms.
const LIGHT_BYTES: usize = 48;

/// Bytes of the moving light uniforms.
pub(crate) const LIVE_BYTES: usize = 32
    + MAX_LIVE_LIGHTS * LIGHT_BYTES
    + MAX_SHADOW_LIGHTS * 6 * 64
    + CLUSTERS_X * CLUSTERS_Y * CLUSTERS_Z * 8
    + SUN_BYTES;

/// Bytes of a sun's: three maps' matrices, then where it is, its light, and how far each map
/// reaches.
const SUN_BYTES: usize = 3 * 64 + 48;

/// A moving light in a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LiveLight {
    /// Which of the map's lights it is, so its shadow can be kept.
    pub id: u32,
    /// Where it is.
    pub origin: Vec3,
    /// How far it reaches.
    pub radius: f32,
    /// Its colour, where 1 is full.
    pub color: [f32; 3],
    /// Which way a spot points, or zero for all round.
    pub spot: Vec3,
    /// The cosine of a spot's half angle.
    pub spot_cosine: f32,
    /// Whether it casts shadows.
    pub shadows: bool,
    /// How much of what it lights bounces on, where 1 is as much as a baked light's does, and 0,
    /// the usual, none (proposal 0153).
    pub bounce: f32,
}

/// Which slice into the view `depth` is in.
#[must_use]
pub fn slice_of(depth: f32) -> usize {
    let scale = slice_scale();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to the slices"
    )]
    let slice = ((depth.max(SLICE_NEAR) / SLICE_NEAR).log2() * scale) as usize;
    slice.min(CLUSTERS_Z - 1)
}

/// Slices per doubling of distance.
#[expect(clippy::cast_precision_loss, reason = "a small count")]
fn slice_scale() -> f32 {
    CLUSTERS_Z as f32 / (SLICE_FAR / SLICE_NEAR).log2()
}

/// Which clusters each light and capsule reaches, as a bit for each in each
/// cluster. A sphere's bounds on the screen stand in for them, which can only add
/// clusters that a pixel's own distance check then leaves dark.
#[must_use]
pub fn cluster_masks(
    view: &View,
    aspect: f32,
    lights: &[LiveLight],
    capsules: &[CapsuleDraw],
) -> Vec<u64> {
    let lit = lights.iter().take(MAX_LIVE_LIGHTS).enumerate();
    let spheres = lit.map(|(bit, light)| (bit, light.origin, light.radius));
    let shaded = capsules.iter().take(MAX_CAPSULES).enumerate();
    let shades = shaded.map(|(bit, capsule)| {
        let half = CAPSULE_REACH * 0.5;
        let middle = capsule.origin - capsule.toward_light * half;
        (MAX_LIVE_LIGHTS + bit, middle, half + capsule.radius * 2.0)
    });
    let mut masks = vec![0; CLUSTERS_X * CLUSTERS_Y * CLUSTERS_Z];
    let [forward, right, up] = rolled_axes(view.yaw, view.pitch, view.roll);
    let scale_y = (4.0 / 3.0) / (view.fov.to_radians() * 0.5).tan();
    let scale_x = scale_y / aspect;
    #[expect(clippy::cast_precision_loss, reason = "small counts")]
    let (across, down) = (CLUSTERS_X as f32, CLUSTERS_Y as f32);
    for (bit, origin, radius) in spheres.chain(shades) {
        let offset = origin - view.origin;
        let depth = offset.dot(forward);
        if depth + radius <= 0.0 {
            continue;
        }
        let first_slice = slice_of(depth - radius);
        let last_slice = slice_of(depth + radius);
        // A sphere round the eye covers the whole view.
        let near = depth - radius;
        let (left, top, right_edge, bottom) = if near <= 1.0 {
            (0, 0, CLUSTERS_X - 1, CLUSTERS_Y - 1)
        } else {
            let (x, y) = (offset.dot(right), offset.dot(up));
            let spread_x = (radius / near) * scale_x;
            let spread_y = (radius / near) * scale_y;
            let (middle_x, middle_y) = (x / depth * scale_x, y / depth * scale_y);
            let column = |value: f32| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "clamped to the clusters"
                )]
                let at = ((value * 0.5 + 0.5).clamp(0.0, 0.999) * across) as usize;
                at
            };
            let row = |value: f32| {
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "clamped to the clusters"
                )]
                let at = ((0.5 - value * 0.5).clamp(0.0, 0.999) * down) as usize;
                at
            };
            (
                column(middle_x - spread_x),
                row(middle_y + spread_y),
                column(middle_x + spread_x),
                row(middle_y - spread_y),
            )
        };
        for slice in first_slice..=last_slice {
            for y in top..=bottom {
                for x in left..=right_edge {
                    let at = (slice * CLUSTERS_Y + y) * CLUSTERS_X + x;
                    masks[at] |= 1 << bit;
                }
            }
        }
    }
    masks
}

/// The matrix of one face of a light's shadow cube, column-major.
#[must_use]
pub fn cube_face(origin: Vec3, face: usize, far: f32) -> [f32; 16] {
    let (forward, up) = match face {
        0 => (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
        1 => (Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
        2 => (Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
        3 => (Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)),
        4 => (Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0)),
        _ => (Vec3::new(0.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 0.0)),
    };
    let right = Vec3::new(
        forward.y * up.z - forward.z * up.y,
        forward.z * up.x - forward.x * up.z,
        forward.x * up.y - forward.y * up.x,
    );
    let near = 2.0;
    let depth = far / (far - near);
    let row = |axis: Vec3, scale: f32, add: f32| {
        [
            axis.x * scale,
            axis.y * scale,
            axis.z * scale,
            -axis.dot(origin) * scale + add,
        ]
    };
    let x = row(right, 1.0, 0.0);
    let y = row(up, 1.0, 0.0);
    let z = row(forward, depth, -near * depth);
    let w = row(forward, 1.0, 0.0);
    [
        x[0], y[0], z[0], w[0], x[1], y[1], z[1], w[1], x[2], y[2], z[2], w[2], x[3], y[3], z[3],
        w[3],
    ]
}

/// The moving light uniforms. `slots` says which shadow slot each light has.
#[must_use]
pub(crate) fn live_bytes(
    view: &View,
    aspect: f32,
    size: (u32, u32),
    lights: &[LiveLight],
    capsules: &[CapsuleDraw],
    slots: &[Option<usize>],
    sun: Option<(SunDraw, SunShadow, Option<f32>)>,
) -> Vec<u8> {
    let mut floats = Vec::with_capacity(LIVE_BYTES / 4);
    let lights = lights.get(..MAX_LIVE_LIGHTS).unwrap_or(lights);
    let [forward, ..] = rolled_axes(view.yaw, view.pitch, view.roll);
    #[expect(clippy::cast_precision_loss, reason = "small counts and sizes")]
    floats.extend([
        size.0 as f32,
        size.1 as f32,
        lights.len() as f32,
        slice_scale(),
        forward.x,
        forward.y,
        forward.z,
        SLICE_NEAR,
    ]);
    let mut matrices = vec![0.0; MAX_SHADOW_LIGHTS * 6 * 16];
    for index in 0..MAX_LIVE_LIGHTS {
        let Some(light) = lights.get(index) else {
            floats.extend([0.0; 12]);
            continue;
        };
        let slot = slots.get(index).copied().flatten();
        #[expect(clippy::cast_precision_loss, reason = "a small count")]
        let first_tile = slot.map_or(-1.0, |slot| (slot * 6) as f32);
        let [red, green, blue] = light.color;
        floats.extend([
            light.origin.x,
            light.origin.y,
            light.origin.z,
            light.radius,
            red,
            green,
            blue,
            first_tile,
            light.spot.x,
            light.spot.y,
            light.spot.z,
            light.spot_cosine,
        ]);
        if let Some(slot) = slot {
            for face in 0..6 {
                let at = (slot * 6 + face) * 16;
                matrices[at..at + 16].copy_from_slice(&cube_face(light.origin, face, light.radius));
            }
        }
    }
    floats.extend(matrices);
    let mut bytes = Vec::with_capacity(LIVE_BYTES);
    for float in floats {
        bytes.extend_from_slice(&float.to_le_bytes());
    }
    for mask in cluster_masks(view, aspect, lights, capsules) {
        bytes.extend_from_slice(&mask.to_le_bytes());
    }
    let mut sun_floats = vec![0.0_f32; SUN_BYTES / 4];
    if let Some((draw, shadow, arc)) = sun {
        for (at, matrix) in shadow.matrices.iter().enumerate() {
            sun_floats[at * 16..at * 16 + 16].copy_from_slice(matrix);
        }
        let toward = draw.toward.normalized();
        sun_floats[48..52].copy_from_slice(&[toward.x, toward.y, toward.z, 1.0]);
        sun_floats[52..55].copy_from_slice(&draw.color);
        // One past where it is along its arc, so that none is none.
        sun_floats[55] = arc.map_or(0.0, |place| place + 1.0);
        sun_floats[56..59].copy_from_slice(&SUN_REACHES);
    }
    for float in sun_floats {
        bytes.extend_from_slice(&float.to_le_bytes());
    }
    bytes
}
