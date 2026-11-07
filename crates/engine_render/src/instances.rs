//! Which copies of shared meshes to draw this frame, grouped so that a mesh's copies are one
//! draw a part (proposal 0140).

use engine_core::math::Vec3;
use engine_map::Map;

use crate::world::InstanceLight;

/// Bytes of one instance: three rows, where its light is, and its styles.
pub const INSTANCE_BYTES: usize = 64;

/// Bytes in a mesh corner's place and normal: three floats and four bytes. A mesh keeps these apart
/// from its UVs, as this is what a moving mesh rewrites each frame.
pub(crate) const PLACE_BYTES: usize = 16;

/// Bytes in a mesh corner's UV.
pub(crate) const UV_BYTES: usize = 8;

/// How much further in a mesh has to come to go back to a finer level, so that something at
/// a boundary doesn't flicker between two.
const COMING_BACK: f32 = 1.1;

/// A moving mesh to draw this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MovingDraw {
    /// Which of the map's moving meshes.
    pub mesh: u32,
    /// What places it.
    pub rows: engine_map::Rows,
    /// Where its joints start in the frame's palette.
    pub first_joint: u32,
    /// How many it has.
    pub joint_count: u32,
    /// Where its morph weights start in the frame's list.
    pub first_weight: u32,
    /// How many it has, or 0 if none of them show.
    pub weight_count: u32,
    /// Its light, as a colour.
    pub light: [f32; 3],
    /// Its middle, in the world, as it's posed.
    pub center: Vec3,
    /// How far it reaches from there.
    pub radius: f32,
}

/// The copies of one level of one mesh that are drawn together.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InstanceBatch {
    /// Which mesh.
    pub mesh: u32,
    /// Which of its levels.
    pub level: u32,
    /// The first of its copies in the instance buffer.
    pub first: u32,
    /// How many there are.
    pub count: u32,
}

/// What choosing levels goes by.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LevelView {
    /// Where the eye is.
    pub eye: Vec3,
    /// How much of the view's height one unit fills at one unit away.
    pub height_scale: f32,
}

/// The level to be at, with `share` of the view's height filled, from `current`. `sizes` are
/// the shares at which each coarser level takes over, and there are `count` levels.
#[must_use]
pub fn choose_level(current: usize, share: f32, sizes: &[f32], count: usize) -> usize {
    let sizes = sizes.get(..count.saturating_sub(1)).unwrap_or(sizes);
    let plain = sizes.iter().filter(|size| share < **size).count();
    if plain >= current {
        return plain;
    }
    sizes
        .iter()
        .filter(|size| share < **size * COMING_BACK)
        .count()
        .min(current)
}

/// How much of the view's height something of `radius` fills at `distance`.
#[must_use]
pub fn height_share(radius: f32, distance: f32, height_scale: f32) -> f32 {
    radius * height_scale / distance.max(radius).max(1.0)
}

/// The six sides of what `matrix` sees, each facing in, from its rows.
#[must_use]
pub fn view_planes(matrix: &[f32; 16]) -> [[f32; 4]; 6] {
    let row = |at: usize| [matrix[at], matrix[4 + at], matrix[8 + at], matrix[12 + at]];
    let [x, y, z, w] = [row(0), row(1), row(2), row(3)];
    let add = |a: [f32; 4], b: [f32; 4]| [a[0] + b[0], a[1] + b[1], a[2] + b[2], a[3] + b[3]];
    let less = |a: [f32; 4], b: [f32; 4]| [a[0] - b[0], a[1] - b[1], a[2] - b[2], a[3] - b[3]];
    [add(w, x), less(w, x), add(w, y), less(w, y), z, less(w, z)].map(|plane| {
        let length = Vec3::new(plane[0], plane[1], plane[2])
            .length()
            .max(f32::EPSILON);
        plane.map(|part| part / length)
    })
}

/// Whether a ball at `center` of `radius` can be seen between `planes`.
#[must_use]
pub fn ball_in_view(planes: &[[f32; 4]; 6], center: Vec3, radius: f32) -> bool {
    planes.iter().all(|plane| {
        plane[0] * center.x + plane[1] * center.y + plane[2] * center.z + plane[3] >= -radius
    })
}

/// One instance's bytes.
pub fn write_instance(rows: &[[f32; 4]; 3], light: &InstanceLight, out: &mut [u8]) {
    let numbers = rows
        .iter()
        .flatten()
        .copied()
        .chain(light.uv)
        .chain([light.step]);
    for (chunk, number) in out.as_chunks_mut::<4>().0.iter_mut().zip(numbers) {
        *chunk = number.to_le_bytes();
    }
    out[60..64].copy_from_slice(&light.styles);
}

/// Writes the copies among `visible` that `planes` can see to `out`, mesh by mesh and level by
/// level, and says where each of those is. `levels` is where each copy is, and is moved on.
pub fn instance_batches(
    map: &Map,
    lights: &[InstanceLight],
    visible: &[u32],
    planes: &[[f32; 4]; 6],
    (view, levels): (&LevelView, &mut [usize]),
    out: &mut [u8],
) -> Vec<InstanceBatch> {
    let mut first_of = Vec::with_capacity(map.shared_meshes.len());
    let mut groups = 0;
    for mesh in &map.shared_meshes {
        first_of.push(groups);
        groups += mesh.levels.len().max(1);
    }
    let mut grouped: Vec<Vec<u32>> = vec![Vec::new(); groups];
    for &number in visible {
        let at = number as usize;
        let (Some(instance), Some(level)) = (map.mesh_instances.get(at), levels.get_mut(at)) else {
            continue;
        };
        let Some(mesh) = map.shared_meshes.get(instance.mesh as usize) else {
            continue;
        };
        let center = instance.place(mesh.center);
        let radius = mesh.radius * instance.stretch();
        if !ball_in_view(planes, center, radius) {
            continue;
        }
        let share = height_share(radius, (center - view.eye).length(), view.height_scale);
        *level = choose_level(*level, share, &mesh.level_sizes, mesh.levels.len());
        grouped[first_of[instance.mesh as usize] + *level].push(number);
    }
    let mut batches = Vec::new();
    let mut slots = out.as_chunks_mut::<INSTANCE_BYTES>().0.iter_mut();
    let mut first = 0;
    for (mesh, (&start, shared)) in (0_u32..).zip(first_of.iter().zip(&map.shared_meshes)) {
        for level in 0..shared.levels.len().max(1) {
            let mut count = 0;
            for &number in &grouped[start + level] {
                let at = number as usize;
                let (Some(instance), Some(light)) = (map.mesh_instances.get(at), lights.get(at))
                else {
                    continue;
                };
                let Some(slot) = slots.next() else {
                    break;
                };
                write_instance(&instance.rows, light, slot.as_mut_slice());
                count += 1;
            }
            if count > 0 {
                let level = u32::try_from(level).unwrap_or(0);
                batches.push(InstanceBatch {
                    mesh,
                    level,
                    first,
                    count,
                });
                first += count;
            }
        }
    }
    batches
}

/// A mesh's corners' places and normals, which is all of a corner that moves.
pub(crate) fn shared_places(mesh: &engine_map::SharedMesh) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(mesh.corners.len() * PLACE_BYTES);
    for corner in &mesh.corners {
        for number in corner.position {
            bytes.extend_from_slice(&number.to_le_bytes());
        }
        bytes.extend(corner.normal.map(i8::to_le_bytes).into_iter().flatten());
        bytes.push(127);
    }
    bytes
}

/// A mesh's corners' UVs, which is what stays put when it moves.
pub(crate) fn shared_uvs(mesh: &engine_map::SharedMesh) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(mesh.corners.len() * UV_BYTES);
    for corner in &mesh.corners {
        for number in corner.uv {
            bytes.extend_from_slice(&number.to_le_bytes());
        }
    }
    bytes
}
