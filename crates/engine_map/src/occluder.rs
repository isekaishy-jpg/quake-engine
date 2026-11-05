//! The shade moving things cast, from their sidecars (proposal 0078), or for
//! alias models, from each pose (proposal 0093).

use engine_core::math::Vec3;

use crate::alias::AliasModel;

/// How many parts a pose has, along its longest side.
const POSE_PARTS: usize = 3;

/// How wide a shade is, for how wide what casts it is.
const SHADE_WIDTH: f32 = 0.35;

/// A ball of a moving thing's shade.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OccluderSphere {
    /// Its middle, in the model's own space.
    pub center: [f32; 3],
    /// How big it is.
    pub radius: f32,
}

/// The shade a brush model casts wherever it goes. The first sphere is the
/// whole of it, and the others are its parts, for up close.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelOccluder {
    /// The model.
    pub model: u32,
    /// Its spheres.
    pub spheres: Vec<OccluderSphere>,
}

/// The spheres of the points in `points`: its middle, and how wide it is across
/// its two longest sides, as what casts a shade is seen from a light.
fn sphere_of(points: &[Vec3]) -> Option<OccluderSphere> {
    let first = *points.first()?;
    let (mut low, mut high) = (first, first);
    for point in points {
        low = Vec3::new(low.x.min(point.x), low.y.min(point.y), low.z.min(point.z));
        high = Vec3::new(
            high.x.max(point.x),
            high.y.max(point.y),
            high.z.max(point.z),
        );
    }
    let middle = (low + high) * 0.5;
    let size = high - low;
    let mut sides = [size.x, size.y, size.z];
    sides.sort_by(f32::total_cmp);
    Some(OccluderSphere {
        center: [middle.x, middle.y, middle.z],
        radius: (sides[1] + sides[2]) * SHADE_WIDTH * 0.5,
    })
}

/// The shade of one pose in the model's own space: the whole of it, then its
/// parts along its longest side. A body standing and one lying down come out
/// differently, which is what a capsule can't do.
#[must_use]
pub fn pose_spheres(model: &AliasModel, pose: usize) -> Vec<OccluderSphere> {
    let Some(vertices) = model.poses.get(pose) else {
        return Vec::new();
    };
    let mut points: Vec<Vec3> = vertices
        .iter()
        .map(|vertex| {
            let [x, y, z] = vertex.position.map(f32::from);
            Vec3::new(
                x * model.scale.x + model.offset.x,
                y * model.scale.y + model.offset.y,
                z * model.scale.z + model.offset.z,
            )
        })
        .collect();
    let Some(whole) = sphere_of(&points) else {
        return Vec::new();
    };
    let along = |point: &Vec3, axis: usize| [point.x, point.y, point.z][axis];
    let spread = |axis: usize| {
        let values = points.iter().map(|point| along(point, axis));
        let (low, high) = values.fold((f32::MAX, f32::MIN), |(low, high), value| {
            (low.min(value), high.max(value))
        });
        high - low
    };
    let longest = (0..3)
        .max_by(|a, b| spread(*a).total_cmp(&spread(*b)))
        .unwrap_or(2);
    points.sort_by(|a, b| along(a, longest).total_cmp(&along(b, longest)));
    let share = points.len().div_ceil(POSE_PARTS).max(1);
    let mut out = vec![whole];
    out.extend(points.chunks(share).filter_map(sphere_of));
    out
}

/// The shade of every pose of `model`, worked out once when it loads. It's
/// cheap enough that it needs no file of its own.
#[must_use]
pub fn pose_occlusion(model: &AliasModel) -> Vec<Vec<OccluderSphere>> {
    (0..model.poses.len())
        .map(|pose| pose_spheres(model, pose))
        .collect()
}
