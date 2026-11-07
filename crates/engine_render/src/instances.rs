//! Which copies of shared meshes to draw this frame, grouped so that a mesh's copies are one
//! draw a part (proposal 0140).

use engine_core::math::Vec3;
use engine_map::Map;

use crate::world::{InstanceLight, Vertex};

/// Bytes of one instance: three rows, where its light is, and its styles.
pub const INSTANCE_BYTES: usize = 64;

/// The copies of one mesh that are drawn together.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InstanceBatch {
    /// Which mesh.
    pub mesh: u32,
    /// The first of its copies in the instance buffer.
    pub first: u32,
    /// How many there are.
    pub count: u32,
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
    out[INSTANCE_BYTES - 4..].copy_from_slice(&light.styles);
}

/// Writes the copies among `visible` that `planes` can see to `out`, mesh by mesh, and says
/// where each mesh's are.
pub fn instance_batches(
    map: &Map,
    lights: &[InstanceLight],
    visible: &[u32],
    planes: &[[f32; 4]; 6],
    out: &mut [u8],
) -> Vec<InstanceBatch> {
    let mut per_mesh: Vec<Vec<u32>> = vec![Vec::new(); map.shared_meshes.len()];
    for &number in visible {
        let Some(instance) = map.mesh_instances.get(number as usize) else {
            continue;
        };
        let Some(mesh) = map.shared_meshes.get(instance.mesh as usize) else {
            continue;
        };
        let center = instance.place(mesh.center);
        if ball_in_view(planes, center, mesh.radius * instance.stretch()) {
            per_mesh[instance.mesh as usize].push(number);
        }
    }
    let mut batches = Vec::new();
    let mut slots = out.as_chunks_mut::<INSTANCE_BYTES>().0.iter_mut();
    let mut first = 0;
    for (mesh, numbers) in (0_u32..).zip(per_mesh) {
        let mut count = 0;
        for number in numbers {
            let at = number as usize;
            let (Some(instance), Some(light)) = (map.mesh_instances.get(at), lights.get(at)) else {
                continue;
            };
            let Some(slot) = slots.next() else {
                break;
            };
            write_instance(&instance.rows, light, slot.as_mut_slice());
            count += 1;
        }
        if count > 0 {
            batches.push(InstanceBatch { mesh, first, count });
            first += count;
        }
    }
    batches
}

/// The vertices of a shared mesh, as the world's are, lit by what its instances say.
#[must_use]
pub(crate) fn shared_vertices(mesh: &engine_map::SharedMesh) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(mesh.corners.len() * Vertex::SIZE);
    for corner in &mesh.corners {
        let [x, y, z] = corner.normal;
        let vertex = Vertex {
            position: corner.position,
            uv: corner.uv,
            normal: [x, y, z, 127],
            ..Vertex::default()
        };
        vertex.write(&mut bytes);
    }
    bytes
}
