//! Meshes: reading them, placing them, and the volume they shade (proposal
//! 0078).

use std::collections::HashMap;
use std::sync::Arc;

use engine_core::math::Vec3;
use engine_format::{Block, Document, Value, to_text};
use engine_map::{
    AttachPoint, Channel, ChannelPath, Clip, Contents, Interpolation, Joint, JointPose, MeshCorner,
    NO_MOVE, OccluderSphere, Rows, SharedMesh, SharedPart, Skeleton, SkeletonPlace,
};

use crate::build::packed_normal;

use crate::convex::ConvexPiece;
use crate::error::CompileError;
use crate::geometry::{Plane3, Point, add_scaled, cross, dot, normalized};
use crate::source::{Brush, BrushFace, MeshPlacement, PointSource, WorkingMap};

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
    /// Which way each corner faces. With none, they're worked out from its
    /// shape, smooth except where it turns sharply.
    pub normals: Vec<Point>,
    /// Its triangles, as corners.
    pub triangles: Vec<[u32; 3]>,
    /// Its triangles at each coarser level of detail, on the same corners.
    pub levels: Vec<Vec<[u32; 3]>>,
    /// The joints that carry each corner, if it has a skin.
    pub joints: Vec<[u8; 4]>,
    /// How much of each corner goes with each of those, out of 255.
    pub weights: Vec<[u8; 4]>,
}

impl MeshPart {
    /// Works out its normals if it has none, so that each corner has its own, for joining it
    /// to something that has them.
    pub(crate) fn give_normals(&mut self) {
        if !self.normals.is_empty() {
            return;
        }
        let worked_out = smooth_normals(self, &self.triangles);
        let (mut points, mut uvs, mut normals) = (Vec::new(), Vec::new(), Vec::new());
        let mut known = HashMap::new();
        let mut triangles = Vec::with_capacity(self.triangles.len());
        for (triangle, facing) in self.triangles.iter().zip(&worked_out) {
            let mut made = [0; 3];
            for at in 0..3 {
                let corner = triangle[at] as usize;
                let key = (
                    self.points[corner].map(f64::to_bits),
                    self.uvs[corner].map(f64::to_bits),
                    facing[at].map(f64::to_bits),
                );
                made[at] = *known.entry(key).or_insert_with(|| {
                    points.push(self.points[corner]);
                    uvs.push(self.uvs[corner]);
                    normals.push(facing[at]);
                    u32::try_from(points.len() - 1).unwrap_or(u32::MAX)
                });
            }
            triangles.push(made);
        }
        self.points = points;
        self.uvs = uvs;
        self.normals = normals;
        self.triangles = triangles;
    }
}

/// A named place on a mesh, in its own space.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshAttach {
    /// What it's called.
    pub name: String,
    /// Where it is.
    pub origin: Point,
    /// Which way it faces.
    pub forward: Point,
    /// What's to its left.
    pub left: Point,
}

/// A mesh as it's stored.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mesh {
    /// Its pieces.
    pub parts: Vec<MeshPart>,
    /// The share of the view's height at which each coarser level takes over, if it's said.
    pub level_sizes: Vec<f32>,
    /// Its named places.
    pub attach: Vec<MeshAttach>,
    /// Its joints and clips, if it moves.
    pub skeleton: Option<Skeleton>,
    /// Boxes for collision, as low and high corners, if an author chose them.
    pub hulls: Vec<[f64; 6]>,
    /// Convex pieces that fit it, for collision where no boxes were chosen. With
    /// neither, its bounds are the one box.
    pub convex: Vec<ConvexPiece>,
}

/// A triangle where a mesh was placed.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedTriangle {
    /// Its corners.
    pub points: [Point; 3],
    /// Their places on the material.
    pub uvs: [[f64; 2]; 3],
    /// Which way they face.
    pub normals: [Point; 3],
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
    /// Whether it's lit from one place, and not by a light map of its own.
    pub by_probe: bool,
    /// Which of the level's placements this is, if it's drawn as an instance.
    pub placement: Option<u32>,
    /// Its named places, where they've ended up.
    pub attach: Vec<AttachPoint>,
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
        let normals = groups::<3>(block, "normals");
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
        let mut levels = Vec::new();
        for level in 1.. {
            let name = format!("level_{level}");
            if block.get(&name).is_none() {
                break;
            }
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "corner numbers are small and checked next"
            )]
            let coarser: Vec<[u32; 3]> = groups::<3>(block, &name)
                .iter()
                .map(|corners| corners.map(|corner| corner as u32))
                .collect();
            if coarser.iter().flatten().any(|c| *c as usize >= known) {
                return Err(CompileError::Bad(
                    "a mesh's level has no such corner".to_owned(),
                ));
            }
            levels.push(coarser);
        }
        if !normals.is_empty() && normals.len() != points.len() {
            return Err(CompileError::Bad(
                "a mesh's normals aren't one for each corner".to_owned(),
            ));
        }
        let bytes = |key: &str| -> Vec<[u8; 4]> {
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "these were bytes going in"
            )]
            groups::<4>(block, key)
                .iter()
                .map(|group| group.map(|part| part as u8))
                .collect()
        };
        let (joints, weights) = (bytes("joints"), bytes("weights"));
        if !joints.is_empty() && (joints.len() != points.len() || weights.len() != points.len()) {
            return Err(CompileError::Bad(
                "a mesh's skin isn't one for each corner".to_owned(),
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
            normals,
            triangles,
            levels,
            joints,
            weights,
        });
    }
    mesh.skeleton = read_skeleton_of(document);
    let last_joint = mesh
        .parts
        .iter()
        .flat_map(|part| part.joints.iter().flatten())
        .max();
    let joint_count = mesh
        .skeleton
        .as_ref()
        .map_or(0, |skeleton| skeleton.joints.len());
    if last_joint.is_some_and(|joint| usize::from(*joint) >= joint_count) {
        return Err(CompileError::Bad(
            "a mesh's skin has no such joint".to_owned(),
        ));
    }
    for block in document.blocks_of("attach") {
        let triple =
            |key: &str, default: Point| groups::<3>(block, key).first().copied().unwrap_or(default);
        mesh.attach.push(MeshAttach {
            name: block.name.clone().unwrap_or_default(),
            origin: triple("origin", [0.0; 3]),
            forward: triple("forward", [1.0, 0.0, 0.0]),
            left: triple("left", [0.0, 1.0, 0.0]),
        });
    }
    #[expect(clippy::cast_possible_truncation, reason = "shares of a screen")]
    for block in document.blocks_of("levels") {
        let sizes = block
            .get("sizes")
            .and_then(Value::numbers)
            .unwrap_or_default();
        mesh.level_sizes = sizes.iter().map(|size| *size as f32).collect();
    }
    for block in document.blocks_of("hull") {
        mesh.hulls.extend(groups::<6>(block, "cuboid"));
    }
    for block in document.blocks_of("convex") {
        let planes = groups::<4>(block, "planes");
        let piece = planes.iter().map(|[x, y, z, far]| ([*x, *y, *z], *far));
        mesh.convex.push(piece.collect());
    }
    Ok(mesh)
}

/// Numbers as a `[f32]`, as a file keeps them.
#[expect(clippy::cast_possible_truncation, reason = "joints store f32")]
fn floats(block: &Block, key: &str) -> Vec<f32> {
    let numbers = block.get(key).and_then(Value::numbers).unwrap_or_default();
    numbers.iter().map(|number| *number as f32).collect()
}

/// Rows from twelve numbers, or what moves nothing.
fn rows_from(numbers: &[f32]) -> Rows {
    let mut rows = NO_MOVE;
    for (slot, number) in rows.iter_mut().flatten().zip(numbers) {
        *slot = *number;
    }
    rows
}

/// A joint's number from a file, where -1 is none.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a joint's number"
)]
fn joint_at(block: &Block, key: &str) -> Option<u16> {
    let number = block.get(key).and_then(Value::number)?;
    (number >= 0.0).then_some(number as u16)
}

/// The skeleton in `document`, if it has one.
fn read_skeleton_of(document: &Document) -> Option<Skeleton> {
    let settings = document.blocks_of("skeleton").next()?;
    #[expect(clippy::cast_possible_truncation, reason = "joints store f32")]
    let reach = settings.get("reach").and_then(Value::number).unwrap_or(0.0) as f32;
    let three = |numbers: Vec<f32>, fallback: [f32; 3]| {
        numbers
            .get(..3)
            .map_or(fallback, |part| [part[0], part[1], part[2]])
    };
    let joints = document
        .blocks_of("joint")
        .map(|block| {
            let turn = floats(block, "rotation");
            Joint {
                name: block.name.clone().unwrap_or_default(),
                parent: joint_at(block, "parent"),
                rest: JointPose {
                    translation: three(floats(block, "translation"), [0.0; 3]),
                    rotation: turn.get(..4).map_or([0.0, 0.0, 0.0, 1.0], |part| {
                        [part[0], part[1], part[2], part[3]]
                    }),
                    scale: three(floats(block, "scale"), [1.0; 3]),
                },
                inverse_bind: rows_from(&floats(block, "inverse_bind")),
            }
        })
        .collect();
    let clips = document
        .blocks_of("clip")
        .map(|block| {
            #[expect(clippy::cast_possible_truncation, reason = "clips store f32")]
            let length = block.get("length").and_then(Value::number).unwrap_or(0.0) as f32;
            let channels = block
                .children_of("channel")
                .map(|channel| Channel {
                    joint: joint_at(channel, "joint").unwrap_or(u16::MAX),
                    path: match channel.get("path").and_then(Value::text) {
                        Some("rotation") => ChannelPath::Rotation,
                        Some("scale") => ChannelPath::Scale,
                        _ => ChannelPath::Translation,
                    },
                    interpolation: match channel.get("between").and_then(Value::text) {
                        Some("step") => Interpolation::Step,
                        Some("cubic") => Interpolation::Cubic,
                        _ => Interpolation::Linear,
                    },
                    times: floats(channel, "times"),
                    values: floats(channel, "values"),
                })
                .collect();
            Clip {
                name: block.name.clone().unwrap_or_default(),
                length,
                channels,
            }
        })
        .collect();
    let places = document
        .blocks_of("place")
        .map(|block| SkeletonPlace {
            name: block.name.clone().unwrap_or_default(),
            joint: joint_at(block, "joint"),
            offset: rows_from(&floats(block, "offset")),
        })
        .collect();
    Some(Skeleton {
        joints,
        clips,
        places,
        reach,
    })
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

    /// Which way `normal` faces once placed. A stretched surface leans the other
    /// way from how it was stretched.
    fn turn(&self, normal: Point) -> Point {
        let mut out = [0.0; 3];
        for ((axis, part), scale) in self.axes.iter().zip(normal).zip(self.scale) {
            let scale = if scale == 0.0 { 1.0 } else { scale };
            out = add_scaled(out, *axis, part / scale);
        }
        normalized(out).unwrap_or(normal)
    }

    /// Which way `direction` points once turned. Stretching doesn't change how a place faces.
    fn aim(&self, direction: Point) -> Point {
        let mut out = [0.0; 3];
        for (axis, part) in self.axes.iter().zip(direction) {
            out = add_scaled(out, *axis, part);
        }
        out
    }

    /// `attach` where this puts it.
    fn place_point(&self, attach: &MeshAttach) -> AttachPoint {
        let [x, y, z] = self.apply(attach.origin);
        let angles = angles_of(self.aim(attach.forward), self.aim(attach.left));
        #[expect(clippy::cast_possible_truncation, reason = "maps store f32")]
        AttachPoint {
            name: attach.name.clone(),
            origin: Vec3::new(x as f32, y as f32, z as f32),
            angles: Vec3::new(angles[0] as f32, angles[1] as f32, angles[2] as f32),
        }
    }

    /// Whether this turns surfaces inside out.
    fn mirrors(&self) -> bool {
        self.scale.iter().filter(|scale| **scale < 0.0).count() % 2 == 1
    }

    /// The brush a convex piece becomes.
    fn convex(&self, piece: &ConvexPiece) -> Brush {
        let faces = piece
            .iter()
            .map(|(normal, far)| {
                let place = self.apply(normal.map(|part| part * far));
                let normal = self.turn(*normal);
                BrushFace {
                    plane: Plane3 {
                        normal,
                        distance: dot(normal, place),
                    },
                    material: "hidden".to_owned(),
                    scale: [1.0, 1.0],
                    offset: [0.0, 0.0],
                    rotation: 0.0,
                }
            })
            .collect();
        Brush {
            faces,
            contents: Contents::Solid,
        }
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

/// The cosine of the sharpest turn that's still drawn smooth.
const SMOOTH_TURN: f64 = 0.766;

/// Which way each corner of each of `triangles` of `part` faces. Triangles meeting at
/// a corner are smoothed together, unless they turn sharply. It goes by where
/// corners are, as a mesh often has two at one place for its UVs.
fn smooth_normals(part: &MeshPart, triangles: &[[u32; 3]]) -> Vec<[Point; 3]> {
    let facing: Vec<(Point, f64)> = triangles
        .iter()
        .map(|triangle| {
            let [a, b, c] = triangle.map(|corner| part.points[corner as usize]);
            let area = cross(add_scaled(b, a, -1.0), add_scaled(c, a, -1.0));
            let size = dot(area, area).sqrt();
            (normalized(area).unwrap_or([0.0, 0.0, 1.0]), size)
        })
        .collect();
    let place = |corner: u32| part.points[corner as usize].map(f64::to_bits);
    let mut meeting: HashMap<[u64; 3], Vec<usize>> = HashMap::new();
    for (number, triangle) in triangles.iter().enumerate() {
        for corner in triangle {
            meeting.entry(place(*corner)).or_default().push(number);
        }
    }
    triangles
        .iter()
        .zip(&facing)
        .map(|(triangle, (own, _))| {
            triangle.map(|corner| {
                let mut sum = [0.0; 3];
                for other in meeting.get(&place(corner)).into_iter().flatten() {
                    let (normal, size) = facing[*other];
                    if dot(normal, *own) >= SMOOTH_TURN {
                        sum = add_scaled(sum, normal, size);
                    }
                }
                normalized(sum).unwrap_or(*own)
            })
        })
        .collect()
}

/// The pitch, yaw and roll in degrees that face along `forward`, with `left` to the left. It's
/// what [`Transform::of`] undone.
#[must_use]
pub fn angles_of(forward: Point, left: Point) -> Point {
    let up = cross(forward, left);
    let pitch = (-forward[2]).clamp(-1.0, 1.0).asin();
    // Straight up or down, yaw and roll are the same turn, so it's all yaw.
    let level = forward[0].hypot(forward[1]) > 1e-6;
    let (yaw, roll) = if level {
        (forward[1].atan2(forward[0]), left[2].atan2(up[2]))
    } else {
        ((-left[0]).atan2(left[1]), 0.0)
    };
    [pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees()]
}

/// The three rows of what `placement` does to a point.
#[must_use]
pub(crate) fn placement_rows(placement: &MeshPlacement) -> [[f32; 4]; 3] {
    let transform = Transform::of(placement);
    let origin = transform.apply([0.0; 3]);
    let column = |axis: usize| {
        let mut point = [0.0; 3];
        point[axis] = 1.0;
        let end = transform.apply(point);
        [end[0] - origin[0], end[1] - origin[1], end[2] - origin[2]]
    };
    let columns = [column(0), column(1), column(2)];
    #[expect(clippy::cast_possible_truncation, reason = "maps store f32")]
    [0, 1, 2].map(|row| {
        [
            columns[0][row] as f32,
            columns[1][row] as f32,
            columns[2][row] as f32,
            origin[row] as f32,
        ]
    })
}

/// When a mesh goes to each coarser level, as a share of the view's height, unless it says.
const USUAL_LEVEL_SIZES: [f32; 3] = [0.25, 0.1, 0.03];

/// The triangles of `part` at `level`, or its coarsest if it has no such level.
fn triangles_at(part: &MeshPart, level: usize) -> &[[u32; 3]] {
    match level {
        0 => &part.triangles,
        _ => part
            .levels
            .get(level - 1)
            .or(part.levels.last())
            .unwrap_or(&part.triangles),
    }
}

/// `mesh` as it's kept for placements to share, with `material` giving a part's texture and
/// what its places on the material are multiplied by. Levels share corners, so a coarser one
/// only costs its indices.
#[must_use]
pub(crate) fn shared_mesh(mesh: &Mesh, material: &mut dyn FnMut(&str) -> (u32, f32)) -> SharedMesh {
    let mut shared = SharedMesh::default();
    let mut known = HashMap::new();
    let (mut low, mut high) = ([f32::MAX; 3], [f32::MIN; 3]);
    let level_count = 1 + mesh
        .parts
        .iter()
        .map(|part| part.levels.len())
        .max()
        .unwrap_or(0);
    let textures: Vec<_> = mesh
        .parts
        .iter()
        .map(|part| material(&part.material))
        .collect();
    for level in 0..level_count {
        let mut parts = Vec::with_capacity(mesh.parts.len());
        for (number, part) in mesh.parts.iter().enumerate() {
            let (texture, across) = textures[number];
            let first = u32::try_from(shared.indices.len()).unwrap_or(u32::MAX);
            let triangles = triangles_at(part, level);
            // Each level's are worked out from its own shape, which is the one that's seen.
            let worked_out = part
                .normals
                .is_empty()
                .then(|| smooth_normals(part, triangles));
            for (triangle_at, triangle) in triangles.iter().enumerate() {
                for (at, corner) in triangle.iter().enumerate() {
                    let corner = *corner as usize;
                    let normal = worked_out
                        .as_ref()
                        .map_or_else(|| part.normals[corner], |normals| normals[triangle_at][at]);
                    #[expect(clippy::cast_possible_truncation, reason = "meshes store f32")]
                    let made = MeshCorner {
                        position: part.points[corner].map(|part| part as f32),
                        uv: part.uvs[corner].map(|part| part as f32 * across),
                        normal: packed_normal(normal),
                    };
                    let key = (
                        made.position.map(f32::to_bits),
                        made.uv.map(f32::to_bits),
                        made.normal,
                    );
                    let index = *known.entry(key).or_insert_with(|| {
                        for axis in 0..3 {
                            low[axis] = low[axis].min(made.position[axis]);
                            high[axis] = high[axis].max(made.position[axis]);
                        }
                        shared.corners.push(made);
                        u32::try_from(shared.corners.len() - 1).unwrap_or(u32::MAX)
                    });
                    shared.indices.push(index);
                }
            }
            let last = u32::try_from(shared.indices.len()).unwrap_or(u32::MAX);
            if last > first {
                parts.push(SharedPart {
                    texture,
                    first,
                    count: last - first,
                });
            }
        }
        shared.levels.push(parts);
    }
    shared.level_sizes = if mesh.level_sizes.is_empty() {
        USUAL_LEVEL_SIZES.to_vec()
    } else {
        mesh.level_sizes.clone()
    };
    if !shared.corners.is_empty() {
        let [low, high] = [low, high].map(|[x, y, z]| Vec3::new(x, y, z));
        shared.center = (low + high) * 0.5;
        shared.radius = (high - low).length() * 0.5;
    }
    shared
}

/// `mesh` where `placement` puts it.
#[must_use]
pub fn place_mesh(mesh: &Mesh, placement: &MeshPlacement) -> PlacedMesh {
    let transform = Transform::of(placement);
    let mut placed = PlacedMesh {
        only_seen: placement.only_seen,
        by_probe: placement.by_probe,
        attach: mesh
            .attach
            .iter()
            .map(|point| transform.place_point(point))
            .collect(),
        ..PlacedMesh::default()
    };
    for part in &mesh.parts {
        let worked_out = part
            .normals
            .is_empty()
            .then(|| smooth_normals(part, &part.triangles));
        for (number, triangle) in part.triangles.iter().enumerate() {
            let mut corners = triangle.map(|corner| corner as usize);
            let mut normals = worked_out.as_ref().map_or_else(
                || corners.map(|corner| part.normals[corner]),
                |normals| normals[number],
            );
            if transform.mirrors() {
                corners.swap(1, 2);
                normals.swap(1, 2);
            }
            placed.triangles.push(PlacedTriangle {
                points: corners.map(|corner| transform.apply(part.points[corner])),
                uvs: corners.map(|corner| part.uvs[corner]),
                normals: normals.map(|normal| transform.turn(normal)),
                material: part.material.clone(),
            });
        }
    }
    if placement.only_seen {
        return placed;
    }
    let all = mesh.parts.iter().flat_map(|part| part.points.iter());
    let whole = bounds(all).map(|(low, high)| [low[0], low[1], low[2], high[0], high[1], high[2]]);
    if mesh.hulls.is_empty() && !mesh.convex.is_empty() {
        placed.hulls = mesh
            .convex
            .iter()
            .map(|piece| transform.convex(piece))
            .collect();
        return placed;
    }
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
    for (number, placement) in (0_u32..).zip(&working.meshes) {
        let loaded = read(&placement.source)?;
        let mut mesh = place_mesh(&loaded.mesh, placement);
        // With one light for all of it, a copy loses nothing by sharing its triangles.
        if placement.by_probe {
            mesh.placement = Some(number);
            working
                .shared
                .insert(placement.source.clone(), Arc::clone(&loaded.mesh));
        }
        placed.push(mesh);
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
    put_attached(working)
}

/// The places of `working` that `copy` of a prefab can see: its own first, so that two copies
/// of a prefab each find their own, and then the rest.
fn find_place<'a>(working: &'a WorkingMap, name: &str, copy: Option<u64>) -> Option<Point> {
    let by_name = |point: &&'a PointSource| point.name == name;
    let same = working
        .points
        .iter()
        .filter(by_name)
        .find(|point| point.copy == copy);
    if let Some(point) = same.or_else(|| working.points.iter().find(by_name)) {
        return Some(point.origin);
    }
    let wanted = |point: &&AttachPoint, of: Option<&str>| {
        point.name == name || of.is_some_and(|of| name == format!("{of}.{}", point.name))
    };
    let placed = working.meshes.iter().zip(&working.placed);
    let on_placed = placed.flat_map(|(placement, placed)| {
        let of = placement.name.as_deref();
        placed.attach.iter().filter(move |point| wanted(point, of))
    });
    let on_entities = working.entities.iter().flat_map(|entity| {
        let attach = entity.placed.iter().flat_map(|placed| &placed.attach);
        attach.filter(|point| wanted(point, None))
    });
    let found = on_placed.chain(on_entities).next()?;
    Some([found.origin.x, found.origin.y, found.origin.z].map(f64::from))
}

/// Puts every entity with an `attach` where that place is.
fn put_attached(working: &mut WorkingMap) -> Result<(), CompileError> {
    let mut places = Vec::new();
    for (number, entity) in working.entities.iter().enumerate() {
        if let Some(name) = entity.text("attach") {
            let place = find_place(working, name, entity.copy).ok_or_else(|| {
                CompileError::Bad(format!(
                    "{} is attached to {name}, which isn't there",
                    entity.kind
                ))
            })?;
            places.push((number, place));
        }
    }
    for (number, place) in places {
        working.entities[number].set_point("origin", place);
    }
    Ok(())
}
