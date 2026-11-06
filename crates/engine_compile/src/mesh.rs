//! Meshes: reading them, placing them, and the volume they shade (proposal
//! 0078).

use std::sync::Arc;

use engine_format::{Block, Document, Value, to_text};
use engine_map::{Contents, OccluderSphere};

use crate::error::CompileError;
use crate::geometry::{Plane3, Point, add_scaled, cross, dot, normalized};
use crate::source::{Brush, BrushFace, MeshPlacement, WorkingMap};

/// A piece of a mesh with one material.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshPart {
    /// What it's called.
    pub name: String,
    /// Its material.
    pub material: String,
    /// Its corners.
    pub points: Vec<Point>,
    /// Each corner's place on the material, from 0 to 1 across.
    pub uvs: Vec<[f64; 2]>,
    /// Its triangles, as corners.
    pub triangles: Vec<[u32; 3]>,
}

/// A mesh as it's stored.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    /// Its pieces.
    pub parts: Vec<MeshPart>,
    /// Boxes for collision, as low and high corners. With none, its bounds are
    /// the one box.
    pub hulls: Vec<[f64; 6]>,
}

/// A triangle where a mesh was placed.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedTriangle {
    /// Its corners.
    pub points: [Point; 3],
    /// Their places on the material.
    pub uvs: [[f64; 2]; 3],
    /// Its material.
    pub material: String,
}

/// A mesh where it was placed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlacedMesh {
    /// What's drawn.
    pub triangles: Vec<PlacedTriangle>,
    /// What's collided with. Light passes these and hits the triangles.
    pub hulls: Vec<Brush>,
    /// The volume it shades, from its sidecar, if it has one.
    pub occluders: Vec<OccluderSphere>,
    /// Whether light goes through it.
    pub only_seen: bool,
}

/// The numbers of a field, in groups of `N`.
fn groups<const N: usize>(block: &Block, key: &str) -> Vec<[f64; N]> {
    let numbers = block.get(key).and_then(Value::numbers).unwrap_or_default();
    let (groups, _) = numbers.as_chunks::<N>();
    groups.to_vec()
}

/// Reads a mesh document.
///
/// # Errors
///
/// Returns an error if it isn't a mesh, or a triangle has no corner.
pub fn read_mesh(document: &Document) -> Result<Mesh, CompileError> {
    if document.kind != "mesh" {
        return Err(CompileError::Bad(format!(
            "expected a mesh, found {}",
            document.kind
        )));
    }
    let mut mesh = Mesh::default();
    for block in document.blocks_of("part") {
        let points = groups::<3>(block, "points");
        let uvs = groups::<2>(block, "uvs");
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "corner numbers are small and checked next"
        )]
        let triangles: Vec<[u32; 3]> = groups::<3>(block, "triangles")
            .iter()
            .map(|corners| corners.map(|corner| corner as u32))
            .collect();
        let known = points.len().min(uvs.len());
        if triangles.iter().flatten().any(|c| *c as usize >= known) {
            return Err(CompileError::Bad(
                "a mesh's triangle has no such corner".to_owned(),
            ));
        }
        mesh.parts.push(MeshPart {
            name: block.name.clone().unwrap_or_default(),
            material: block
                .get("material")
                .and_then(Value::text)
                .unwrap_or_default()
                .to_owned(),
            points,
            uvs,
            triangles,
        });
    }
    for block in document.blocks_of("hull") {
        mesh.hulls.extend(groups::<6>(block, "cuboid"));
    }
    Ok(mesh)
}

/// The low and high corners of `points`.
fn bounds<'a>(points: impl Iterator<Item = &'a Point>) -> Option<(Point, Point)> {
    let mut found = None;
    for point in points {
        let (mut low, mut high) = found.unwrap_or((*point, *point));
        for axis in 0..3 {
            low[axis] = low[axis].min(point[axis]);
            high[axis] = high[axis].max(point[axis]);
        }
        found = Some((low, high));
    }
    found
}

/// Turns and scales points into place.
#[derive(Clone, Copy, Debug)]
struct Transform {
    /// Where the mesh's axes point, once turned.
    axes: [Point; 3],
    /// How much each is stretched.
    scale: [f64; 3],
    /// Where its origin goes.
    origin: Point,
}

impl Transform {
    /// The transform of `placement`, from its pitch, yaw and roll in degrees.
    fn of(placement: &MeshPlacement) -> Self {
        let [pitch, yaw, roll] = placement.angles.map(f64::to_radians);
        let (sin_pitch, cos_pitch) = pitch.sin_cos();
        let (sin_yaw, cos_yaw) = yaw.sin_cos();
        let (sin_roll, cos_roll) = roll.sin_cos();
        let forward = [cos_pitch * cos_yaw, cos_pitch * sin_yaw, -sin_pitch];
        let left = [
            sin_roll * sin_pitch * cos_yaw - cos_roll * sin_yaw,
            sin_roll * sin_pitch * sin_yaw + cos_roll * cos_yaw,
            sin_roll * cos_pitch,
        ];
        Self {
            axes: [forward, left, cross(forward, left)],
            scale: placement.scale,
            origin: placement.origin,
        }
    }

    /// Where `point` goes.
    fn apply(&self, point: Point) -> Point {
        let mut out = self.origin;
        for ((axis, part), scale) in self.axes.iter().zip(point).zip(self.scale) {
            out = add_scaled(out, *axis, part * scale);
        }
        out
    }

    /// Whether this turns surfaces inside out.
    fn mirrors(&self) -> bool {
        self.scale.iter().filter(|scale| **scale < 0.0).count() % 2 == 1
    }

    /// The brush a box becomes.
    fn hull(&self, cuboid: &[f64; 6]) -> Brush {
        let [low_x, low_y, low_z, high_x, high_y, high_z] = *cuboid;
        let middle = self.apply([
            (low_x + high_x) * 0.5,
            (low_y + high_y) * 0.5,
            (low_z + high_z) * 0.5,
        ]);
        let sides = [
            ([low_x, low_y, low_z], [0, 0, 1], [0, 1, 0]),
            ([high_x, low_y, low_z], [0, 1, 0], [0, 0, 1]),
            ([low_x, low_y, low_z], [1, 0, 0], [0, 0, 1]),
            ([low_x, high_y, low_z], [0, 0, 1], [1, 0, 0]),
            ([low_x, low_y, low_z], [0, 1, 0], [1, 0, 0]),
            ([low_x, low_y, high_z], [1, 0, 0], [0, 1, 0]),
        ];
        let faces = sides
            .iter()
            .filter_map(|(corner, first, second)| {
                let along = |edge: [i32; 3]| {
                    self.apply([
                        corner[0] + f64::from(edge[0]) * (high_x - low_x),
                        corner[1] + f64::from(edge[1]) * (high_y - low_y),
                        corner[2] + f64::from(edge[2]) * (high_z - low_z),
                    ])
                };
                let start = self.apply(*corner);
                let (a, b) = (along(*first), along(*second));
                let edge = |to: Point| add_scaled(to, start, -1.0);
                let mut normal = normalized(cross(edge(a), edge(b)))?;
                if dot(normal, add_scaled(start, middle, -1.0)) < 0.0 {
                    normal = normal.map(|part| -part);
                }
                Some(BrushFace {
                    plane: Plane3 {
                        normal,
                        distance: dot(normal, start),
                    },
                    material: "hidden".to_owned(),
                    scale: [1.0, 1.0],
                    offset: [0.0, 0.0],
                    rotation: 0.0,
                })
            })
            .collect();
        Brush {
            faces,
            contents: Contents::Solid,
        }
    }
}

/// `mesh` where `placement` puts it.
#[must_use]
pub fn place_mesh(mesh: &Mesh, placement: &MeshPlacement) -> PlacedMesh {
    let transform = Transform::of(placement);
    let mut placed = PlacedMesh {
        only_seen: placement.only_seen,
        ..PlacedMesh::default()
    };
    for part in &mesh.parts {
        for triangle in &part.triangles {
            let mut corners = triangle.map(|corner| corner as usize);
            if transform.mirrors() {
                corners.swap(1, 2);
            }
            placed.triangles.push(PlacedTriangle {
                points: corners.map(|corner| transform.apply(part.points[corner])),
                uvs: corners.map(|corner| part.uvs[corner]),
                material: part.material.clone(),
            });
        }
    }
    if placement.only_seen {
        return placed;
    }
    let all = mesh.parts.iter().flat_map(|part| part.points.iter());
    let whole = bounds(all).map(|(low, high)| [low[0], low[1], low[2], high[0], high[1], high[2]]);
    let hulls = if mesh.hulls.is_empty() {
        whole.into_iter().collect()
    } else {
        mesh.hulls.clone()
    };
    placed.hulls = hulls.iter().map(|hull| transform.hull(hull)).collect();
    placed
}

/// What a mesh's sphere is, with `points` for its corners.
#[expect(clippy::cast_precision_loss, reason = "corner counts are small")]
#[expect(clippy::cast_possible_truncation, reason = "spheres store f32")]
fn sphere_of(points: &[&Point]) -> Option<OccluderSphere> {
    if points.is_empty() {
        return None;
    }
    let mut middle = [0.0; 3];
    for point in points {
        middle = add_scaled(middle, **point, 1.0 / points.len() as f64);
    }
    // The root of the mean square keeps a thin spike from swelling the volume.
    let spread: f64 = points
        .iter()
        .map(|point| {
            let offset = add_scaled(**point, middle, -1.0);
            dot(offset, offset)
        })
        .sum::<f64>()
        / points.len() as f64;
    Some(OccluderSphere {
        center: [middle[0] as f32, middle[1] as f32, middle[2] as f32],
        radius: spread.sqrt() as f32,
    })
}

/// The volume a mesh shades: one averaged sphere, then a sphere for each part
/// when it has several, for detail up close.
#[must_use]
pub fn occluders_of(mesh: &Mesh) -> Vec<OccluderSphere> {
    let every: Vec<&Point> = mesh.parts.iter().flat_map(|part| &part.points).collect();
    let mut spheres: Vec<OccluderSphere> = sphere_of(&every).into_iter().collect();
    if mesh.parts.len() > 1 {
        for part in &mesh.parts {
            let points: Vec<&Point> = part.points.iter().collect();
            spheres.extend(sphere_of(&points));
        }
    }
    spheres
}

/// A sidecar of `spheres`, as text.
#[must_use]
pub fn write_occlusion(spheres: &[OccluderSphere]) -> String {
    let mut document = Document::new("occlusion");
    for sphere in spheres {
        let mut block = Block::new("sphere");
        let [x, y, z] = sphere.center.map(f64::from);
        block.set(
            "center",
            Value::List(vec![Value::Number(x), Value::Number(y), Value::Number(z)]),
        );
        block.set("radius", Value::Number(f64::from(sphere.radius)));
        document.blocks.push(block);
    }
    to_text(&document)
}

/// Reads a sidecar.
#[must_use]
#[expect(clippy::cast_possible_truncation, reason = "spheres store f32")]
pub fn read_occlusion(document: &Document) -> Vec<OccluderSphere> {
    document
        .blocks_of("sphere")
        .filter_map(|block| {
            let center = block.get("center").and_then(Value::numbers)?;
            let [x, y, z] = center.as_slice() else {
                return None;
            };
            Some(OccluderSphere {
                center: [*x as f32, *y as f32, *z as f32],
                radius: block.get("radius").and_then(Value::number)? as f32,
            })
        })
        .collect()
}

/// A mesh and its sidecar, as a level asks for them.
#[derive(Clone, Debug, Default)]
pub struct LoadedMesh {
    /// The mesh.
    pub mesh: Arc<Mesh>,
    /// Its sidecar's volume, or nothing with no sidecar.
    pub occluders: Vec<OccluderSphere>,
}

/// Places the meshes of `working`, reading them with `read`.
///
/// # Errors
///
/// Returns an error if one can't be read.
pub fn resolve_meshes(
    working: &mut WorkingMap,
    read: &mut dyn FnMut(&str) -> Result<LoadedMesh, CompileError>,
) -> Result<(), CompileError> {
    let mut placed = Vec::with_capacity(working.meshes.len());
    for placement in &working.meshes {
        let loaded = read(&placement.source)?;
        placed.push(place_mesh(&loaded.mesh, placement));
    }
    working.placed = placed;
    for entity in &mut working.entities {
        if let Some(placement) = &entity.mesh {
            let loaded = read(&placement.source)?;
            let mut mesh = place_mesh(&loaded.mesh, placement);
            let transform = Transform::of(placement);
            mesh.occluders = loaded
                .occluders
                .iter()
                .map(|sphere| {
                    let [x, y, z] = sphere.center.map(f64::from);
                    let at = transform.apply([x, y, z]);
                    let widest = placement.scale.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
                    #[expect(clippy::cast_possible_truncation, reason = "spheres store f32")]
                    OccluderSphere {
                        center: at.map(|part| part as f32),
                        radius: sphere.radius * widest as f32,
                    }
                })
                .collect();
            entity.placed = Some(mesh);
        }
    }
    Ok(())
}
