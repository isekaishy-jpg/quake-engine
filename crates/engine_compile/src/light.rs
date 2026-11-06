//! The light compiler: fixed lights baked into the map (proposal 0072).
//!
//! Direct and bounced light stay on separate layers, so moving things can
//! block one without the other. Each sample also records where its light
//! came from, for specular and normal maps under baked light.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use engine_core::math::Vec3;
use engine_map::{
    AlphaMode, BOUNCE_STYLE, CRISP_STYLES, Face, FaceLight, LightTrack, Map, Material, MegaImage,
    MipLevel, ProbeGrid, RESIDENT, TextureKind,
};

use crate::charts::{Chart, map_charts};
use crate::error::CompileError;
use crate::geometry::{Plane3, Point, add_scaled, cross, dot, normalized};
use crate::mega::bake_mega;
use crate::mesh::PlacedMesh;
use crate::nav::build_nav;
use crate::ray_grid::RayGrid;
use crate::source::{PaintStroke, WorkingMap};
use crate::tree::{Child, PlaneSet, TreeBuilder};
use crate::visibility::bake_visibility;

/// World units between light samples, unless a map says otherwise.
const DEFAULT_SPACING: f64 = 16.0;

/// How far a sample sits off its face, so it doesn't shadow itself.
const SURFACE_OFFSET: f64 = 0.5;

/// A sun's rays start this far away.
const SUN_DISTANCE: f64 = 16_384.0;

/// The most layers a face can have, as the renderer reads them.
const MAX_LAYERS: usize = 4;

/// How much the angle light strikes at counts, unless a light says. All of it
/// leaves a circle on each wall at a corner, with nothing between them.
const DEFAULT_ANGLE_SCALE: f64 = 0.5;

/// The most surface samples bounced light gathers from.
const MAX_BOUNCE_SOURCES: usize = 1024;

/// The most a tile keeps for that. A point gathers from nine tiles, so this is
/// about as many as a map without tiles has, however many tiles there are.
const MAX_TILE_BOUNCE_SOURCES: usize = 128;

/// How many items a thread takes at a time.
const PARALLEL_BATCH: usize = 16;

/// How far apart, in samples, bounced light is worked out. It's soft, so what's
/// between can be drawn from what's round it, and it's most of a bake.
const BOUNCE_STRIDE: usize = 2;

/// How many probes apart a grid sits, in world units.
const PROBE_SPACING: f64 = 128.0;

/// The directions a probe looks in.
const PROBE_SIDES: [Point; 6] = [
    [1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
];

/// Reads an image by path.
pub type ImageReader<'a> = dyn FnMut(&str) -> Result<Arc<Vec<MipLevel>>, CompileError> + 'a;

/// A fixed light.
#[derive(Clone, Debug)]
struct BakeLight {
    /// Where it is.
    origin: Point,
    /// Its colour, from 0 to 1.
    color: [f64; 3],
    /// How far it reaches, and how bright it is at its centre.
    intensity: f64,
    /// Its light style.
    style: u8,
    /// How wide it is, for soft shadows, or 0 for a point.
    size: f64,
    /// Which way a spot points.
    direction: Option<Point>,
    /// A spot's half angle, as a cosine.
    cone: f64,
    /// A picture it projects.
    cookie: Option<Arc<Vec<MipLevel>>>,
    /// A falloff by angle, from its axis, as evenly spaced brightnesses.
    profile: Vec<f64>,
    /// Whether it's the sun, shining from infinitely far along `direction`.
    sun: bool,
    /// Whether its shadows stay sharp however coarse the light map is.
    crisp: bool,
    /// How much the angle light strikes at counts, from 0 to 1 (proposal 0098).
    angle_scale: f64,
}

/// A see-through surface that colours light passing through it.
struct Glass {
    /// Its plane.
    plane: Plane3,
    /// Its corners.
    points: Vec<Point>,
    /// Its texture axes.
    axes: [(Point, f64); 2],
    /// The picture of what it lets through.
    image: Arc<Vec<MipLevel>>,
}

impl Glass {
    /// Its low and high corners.
    fn bounds(&self) -> (Point, Point) {
        let mut low = [f64::INFINITY; 3];
        let mut high = [f64::NEG_INFINITY; 3];
        for point in &self.points {
            for axis in 0..3 {
                low[axis] = low[axis].min(point[axis]);
                high[axis] = high[axis].max(point[axis]);
            }
        }
        (low, high)
    }
}

/// What light can hit.
struct Scene<'a> {
    /// Solid brushes, as a tree.
    tree: TreeBuilder,
    /// The tree's root.
    root: Child,
    /// Its planes.
    planes: PlaneSet,
    /// See-through surfaces.
    glass: Vec<Glass>,
    /// Where the glass is.
    glass_grid: RayGrid,
    /// Meshes, which block light by their triangles.
    meshes: Vec<MeshShadow>,
    /// Where the meshes are.
    mesh_grid: RayGrid,
    /// How high the world goes. Nothing above can stop the sun.
    top: f64,
    /// How far stamped faces lift their samples, by their first point.
    lifts: HashMap<u32, Vec3>,
    /// The fixed lights.
    lights: &'a [BakeLight],
}

/// A mesh's triangles, with bounds to pass most rays by.
struct MeshShadow {
    /// Its low corner.
    low: Point,
    /// Its high corner.
    high: Point,
    /// Its triangles.
    triangles: Vec<[Point; 3]>,
}

impl MeshShadow {
    /// The shadow of `mesh`, if it has triangles.
    fn of(mesh: &PlacedMesh) -> Option<Self> {
        let first = mesh.triangles.first()?.points[0];
        let (mut low, mut high) = (first, first);
        for point in mesh.triangles.iter().flat_map(|triangle| &triangle.points) {
            for axis in 0..3 {
                low[axis] = low[axis].min(point[axis]);
                high[axis] = high[axis].max(point[axis]);
            }
        }
        let triangles = mesh
            .triangles
            .iter()
            .map(|triangle| triangle.points)
            .collect();
        Some(Self {
            low,
            high,
            triangles,
        })
    }

    /// Whether the segment from `a` to `b` could touch these bounds.
    fn near(&self, a: Point, b: Point) -> bool {
        let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
        for axis in 0..3 {
            let change = b[axis] - a[axis];
            if change.abs() < 1e-9 {
                if a[axis] < self.low[axis] || a[axis] > self.high[axis] {
                    return false;
                }
                continue;
            }
            let first = (self.low[axis] - a[axis]) / change;
            let second = (self.high[axis] - a[axis]) / change;
            enter = enter.max(first.min(second));
            leave = leave.min(first.max(second));
        }
        enter <= leave
    }

    /// Whether the segment from `a` to `b` passes through a triangle, away
    /// from its ends, so a surface doesn't shade itself.
    fn hit(&self, a: Point, b: Point) -> bool {
        let ray = add_scaled(b, a, -1.0);
        self.near(a, b)
            && self.triangles.iter().any(|triangle| {
                let first = add_scaled(triangle[1], triangle[0], -1.0);
                let second = add_scaled(triangle[2], triangle[0], -1.0);
                let across = cross(ray, second);
                let determinant = dot(first, across);
                if determinant.abs() < 1e-9 {
                    return false;
                }
                let inverse = 1.0 / determinant;
                let offset = add_scaled(a, triangle[0], -1.0);
                let u = dot(offset, across) * inverse;
                if !(0.0..=1.0).contains(&u) {
                    return false;
                }
                let up = cross(offset, first);
                let v = dot(ray, up) * inverse;
                if v < 0.0 || u + v > 1.0 {
                    return false;
                }
                let along = dot(second, up) * inverse;
                along > 0.001 && along < 0.999
            })
    }
}

/// Whether anything solid is between `a` and `b`: brushes or meshes.
fn shadowed(scene: &Scene<'_>, a: Point, b: Point) -> bool {
    blocked(scene, scene.root, a, b)
        || scene
            .mesh_grid
            .any_near(a, b, |index| scene.meshes[index].hit(a, b))
}

/// Whether `child` has solid anywhere between `a` and `b`.
fn blocked(scene: &Scene<'_>, child: Child, a: Point, b: Point) -> bool {
    let Child::Node(index) = child else {
        return child == Child::Leaf(engine_map::Contents::Solid);
    };
    let node = scene.tree.nodes[index as usize];
    let plane = scene.planes.planes[node.plane as usize];
    let (front, back) = (plane.side(a), plane.side(b));
    if front >= 0.0 && back >= 0.0 {
        return blocked(scene, node.front, a, b);
    }
    if front < 0.0 && back < 0.0 {
        return blocked(scene, node.back, a, b);
    }
    let share = front / (front - back);
    let middle = [
        a[0] + (b[0] - a[0]) * share,
        a[1] + (b[1] - a[1]) * share,
        a[2] + (b[2] - a[2]) * share,
    ];
    let (near, far) = if front >= 0.0 {
        (node.front, node.back)
    } else {
        (node.back, node.front)
    };
    blocked(scene, near, a, middle) || blocked(scene, far, middle, b)
}

/// A texel of `image`, as colour and alpha from 0 to 1, wrapping.
fn texel(image: &[MipLevel], u: f64, v: f64) -> [f64; 4] {
    let Some(mip) = image.first() else {
        return [1.0; 4];
    };
    let wrap = |value: f64, size: u32| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "wrapped into the image first"
        )]
        let at = (value.rem_euclid(1.0) * f64::from(size)) as usize;
        at.min(size.saturating_sub(1) as usize)
    };
    let at = (wrap(v, mip.height) * mip.width as usize + wrap(u, mip.width)) * 4;
    let part = |offset: usize| {
        mip.rgba
            .get(at + offset)
            .map_or(1.0, |byte| f64::from(*byte) / 255.0)
    };
    [part(0), part(1), part(2), part(3)]
}

/// How much of a ray from `a` to `b` gets through the glass in its way, as
/// colour. Dark lead between the panes stops it, as any solid would.
fn through_glass(scene: &Scene<'_>, a: Point, b: Point) -> [f64; 3] {
    let mut out = [1.0; 3];
    scene.glass_grid.any_near(a, b, |index| {
        let glass = &scene.glass[index];
        let (front, back) = (glass.plane.side(a), glass.plane.side(b));
        if (front > 0.0) == (back > 0.0) {
            return false;
        }
        let share = front / (front - back);
        let hit = [
            a[0] + (b[0] - a[0]) * share,
            a[1] + (b[1] - a[1]) * share,
            a[2] + (b[2] - a[2]) * share,
        ];
        let inside = (0..glass.points.len()).all(|index| {
            let from = glass.points[index];
            let to = glass.points[(index + 1) % glass.points.len()];
            let edge = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
            let reach = [hit[0] - from[0], hit[1] - from[1], hit[2] - from[2]];
            dot(cross(edge, reach), glass.plane.normal) >= -1e-6
        });
        if !inside {
            return false;
        }
        let size = glass.image.first().map_or([1.0, 1.0], |mip| {
            [f64::from(mip.width), f64::from(mip.height)]
        });
        let u = (dot(hit, glass.axes[0].0) + glass.axes[0].1) / size[0];
        let v = (dot(hit, glass.axes[1].0) + glass.axes[1].1) / size[1];
        let color = texel(&glass.image, u, v);
        for (part, tint) in out.iter_mut().zip(color) {
            *part *= tint;
        }
        false
    });
    out
}

/// A sequence of fixed offsets on a disc, so a soft light stays repeatable.
const DISC: [[f64; 2]; 8] = [
    [0.0, 0.0],
    [0.7, 0.1],
    [-0.5, 0.5],
    [-0.1, -0.7],
    [0.4, -0.45],
    [-0.75, -0.2],
    [0.2, 0.75],
    [0.85, -0.35],
];

/// The light `light` puts on a point with `normal`, and the direction it comes
/// from.
fn light_on(
    scene: &Scene<'_>,
    light: &BakeLight,
    point: Point,
    normal: Point,
    shadows: bool,
) -> ([f64; 3], Point) {
    let none = ([0.0; 3], [0.0; 3]);
    let to_light = if light.sun {
        light.direction.unwrap_or([0.0, 0.0, 1.0])
    } else {
        [
            light.origin[0] - point[0],
            light.origin[1] - point[1],
            light.origin[2] - point[2],
        ]
    };
    let distance = dot(to_light, to_light).sqrt();
    let Some(direction) = normalized(to_light) else {
        return none;
    };
    let facing = dot(direction, normal);
    if facing <= 0.0 {
        return none;
    }
    let mut strength = if light.sun {
        light.intensity
    } else {
        light.intensity - distance
    };
    if strength <= 0.0 {
        return none;
    }
    strength *= 1.0 - light.angle_scale + light.angle_scale * facing;
    if let (Some(axis), false) = (light.direction, light.sun) {
        let along = -dot(direction, axis);
        if along < light.cone {
            return none;
        }
        let edge = ((along - light.cone) / (1.0 - light.cone).max(1e-6)).clamp(0.0, 1.0);
        strength *= edge.sqrt();
        if !light.profile.is_empty() {
            let angle = along.clamp(-1.0, 1.0).acos() / std::f64::consts::PI;
            #[expect(clippy::cast_precision_loss, reason = "a short list")]
            let at = angle * (light.profile.len() - 1) as f64;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "angle is between 0 and 1"
            )]
            let index = (at as usize).min(light.profile.len() - 1);
            strength *= light.profile[index];
        }
    }
    let mut color = light.color;
    if let (Some(cookie), Some(axis)) = (&light.cookie, light.direction) {
        let side = normalized(cross(axis, [0.0, 0.0, 1.0])).unwrap_or([1.0, 0.0, 0.0]);
        let up = cross(side, axis);
        let along = (-dot(direction, axis)).max(1e-3);
        let spread = (1.0 - light.cone * light.cone).max(1e-6).sqrt() / light.cone.max(1e-3);
        let u = dot(direction, side) / along / spread * 0.5 + 0.5;
        let v = dot(direction, up) / along / spread * 0.5 + 0.5;
        let sample = texel(cookie, u, v);
        for (part, tint) in color.iter_mut().zip(sample) {
            *part *= tint;
        }
    }
    let samples = if light.size > 0.0 { DISC.len() } else { 1 };
    let side = normalized(cross(direction, [0.3, 0.2, 0.9])).unwrap_or([1.0, 0.0, 0.0]);
    let up = cross(side, direction);
    let start = add_scaled(point, normal, SURFACE_OFFSET);
    let mut seen = [0.0; 3];
    for offset in DISC.iter().take(samples) {
        let source = if light.sun {
            // Only as far as the top of the world: a longer ray has to cross
            // the whole map's planes for nothing.
            let reach = (scene.top + 64.0 - start[2]) / direction[2].max(1e-3);
            add_scaled(start, direction, reach.clamp(64.0, SUN_DISTANCE))
        } else {
            light.origin
        };
        let target = add_scaled(
            add_scaled(source, side, offset[0] * light.size),
            up,
            offset[1] * light.size,
        );
        if !shadows {
            for part in &mut seen {
                *part += 1.0;
            }
        } else if !shadowed(scene, start, target) {
            let tint = through_glass(scene, start, target);
            for (part, value) in seen.iter_mut().zip(tint) {
                *part += value;
            }
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "eight at most")]
    let share = strength / samples as f64;
    (
        [
            seen[0] * color[0] * share,
            seen[1] * color[1] * share,
            seen[2] * color[2] * share,
        ],
        direction,
    )
}

/// The next style free for keyframes, after the scripts' switchable range.
const FIRST_KEYFRAME_STYLE: u8 = 48;

/// The last style free for keyframes.
const LAST_KEYFRAME_STYLE: u8 = 59;

/// How many visibility samples cross a light sample, along each side, to find
/// a crisp shadow's edge.
const EDGE_DETAIL: i32 = 4;

/// How far an edge is looked for, in those samples.
const EDGE_REACH: i32 = 6;

/// The keyframed lights of `working`, as tracks. Their styles follow the same
/// order as the bake gives them.
#[must_use]
pub fn light_tracks(working: &WorkingMap) -> Vec<LightTrack> {
    let mut tracks = Vec::new();
    let mut next = FIRST_KEYFRAME_STYLE;
    for entity in working
        .entities
        .iter()
        .filter(|e| e.kind.starts_with("light"))
    {
        let field = entity.fields.iter().find(|(name, _)| name == "keyframes");
        let Some(keys) = field.and_then(|(_, value)| value.numbers()) else {
            continue;
        };
        let count = u8::try_from(keys.as_chunks::<3>().0.len()).unwrap_or(0);
        #[expect(clippy::cast_possible_truncation, reason = "tracks store f32")]
        tracks.push(LightTrack {
            first_style: next,
            count,
            period: entity.number("period").unwrap_or(4.0) as f32,
        });
        next = next.saturating_add(count).min(LAST_KEYFRAME_STYLE);
    }
    tracks
}

/// Reads the fixed lights. A light with `keyframes` gives one baked light at
/// each, each on its own style, so its motion can be blended at runtime.
fn read_lights(
    working: &WorkingMap,
    images: &mut ImageReader<'_>,
) -> Result<Vec<BakeLight>, CompileError> {
    let mut lights = Vec::new();
    let mut next_style = FIRST_KEYFRAME_STYLE;
    for entity in working
        .entities
        .iter()
        .filter(|e| e.kind.starts_with("light"))
    {
        let origin = entity.point("origin").unwrap_or([0.0; 3]);
        let cone_degrees = entity.number("spot_angle").unwrap_or(0.0);
        let cookie = entity.text("cookie").map(&mut *images).transpose()?;
        let profile = entity
            .fields
            .iter()
            .find(|(name, _)| name == "profile")
            .and_then(|(_, value)| value.numbers())
            .unwrap_or_default();
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "styles are small numbers"
        )]
        let style = entity.number("style").unwrap_or(0.0).clamp(0.0, 47.0) as u8;
        let base = BakeLight {
            origin,
            color: entity.point("color").unwrap_or([1.0, 1.0, 1.0]),
            intensity: entity
                .number("intensity")
                .or_else(|| entity.number("light"))
                .unwrap_or(300.0),
            style,
            size: entity.number("size").unwrap_or(0.0),
            direction: entity.point("spot_direction").and_then(normalized),
            cone: cone_degrees.to_radians().cos(),
            cookie,
            profile,
            sun: false,
            crisp: entity.is_on("crisp"),
            angle_scale: entity
                .number("angle_scale")
                .unwrap_or(DEFAULT_ANGLE_SCALE)
                .clamp(0.0, 1.0),
        };
        let keyframes = entity
            .fields
            .iter()
            .find(|(name, _)| name == "keyframes")
            .and_then(|(_, value)| value.numbers());
        if let Some(keyframes) = keyframes {
            for key in keyframes.as_chunks::<3>().0 {
                let mut light = base.clone();
                light.origin = *key;
                light.style = next_style;
                next_style = next_style.saturating_add(1).min(LAST_KEYFRAME_STYLE);
                lights.push(light);
            }
        } else {
            lights.push(base);
        }
    }
    Ok(lights)
}

/// The world position of texture coordinates on a face.
struct FaceSpace {
    /// The face's plane.
    plane: Plane3,
    /// How texture coordinates relate to the world.
    rows: [[f64; 4]; 3],
}

/// The inverse of a 3 by 3 system, solved for `rhs`.
fn solve(rows: &[[f64; 4]; 3], rhs: [f64; 3]) -> Option<Point> {
    let a = [rows[0][0], rows[0][1], rows[0][2]];
    let b = [rows[1][0], rows[1][1], rows[1][2]];
    let c = [rows[2][0], rows[2][1], rows[2][2]];
    let det = dot(a, cross(b, c));
    if det.abs() < 1e-12 {
        return None;
    }
    let bc = cross(b, c);
    let ca = cross(c, a);
    let ab = cross(a, b);
    Some([
        (rhs[0] * bc[0] + rhs[1] * ca[0] + rhs[2] * ab[0]) / det,
        (rhs[0] * bc[1] + rhs[1] * ca[1] + rhs[2] * ab[1]) / det,
        (rhs[0] * bc[2] + rhs[1] * ca[2] + rhs[2] * ab[2]) / det,
    ])
}

/// A point from a map's narrower type.
fn wide(point: Vec3) -> Point {
    [f64::from(point.x), f64::from(point.y), f64::from(point.z)]
}

/// A face's plane and texture space.
fn face_space(map: &Map, face: &Face) -> Option<FaceSpace> {
    let first = face.first_point as usize;
    let points = map.points.get(first..first + face.point_count as usize)?;
    let a = wide(*points.first()?);
    let mut normal = None;
    for index in 1..points.len().saturating_sub(1) {
        let b = wide(points[index]);
        let c = wide(points[index + 1]);
        let edge = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let next = [c[0] - b[0], c[1] - b[1], c[2] - b[2]];
        if let Some(found) = normalized(cross(edge, next)) {
            normal = Some(found);
            break;
        }
    }
    let normal = normal?;
    let row = |axis: usize| {
        let texel = face.axes[axis];
        [
            f64::from(texel.axis.x),
            f64::from(texel.axis.y),
            f64::from(texel.axis.z),
            f64::from(texel.offset),
        ]
    };
    Some(FaceSpace {
        plane: Plane3 {
            normal,
            distance: dot(normal, a),
        },
        rows: [row(0), row(1), [normal[0], normal[1], normal[2], 0.0]],
    })
}

/// One face's samples before they're laid out.
struct FaceBake {
    /// Its layers: a style and each sample's colour.
    layers: Vec<(u8, Vec<[f64; 3]>)>,
    /// Where its strongest light comes from, per sample.
    directions: Vec<Point>,
    /// Where it sits.
    light: FaceLight,
    /// Each sample's position.
    points: Vec<Point>,
    /// The face's normal.
    normal: Point,
    /// The face's average colour, for what it bounces.
    albedo: [f64; 3],
    /// How much surface it has.
    area: f64,
    /// How far each sample is from the edge of each crisp shadow, by its style.
    edges: Vec<(u8, Vec<u8>)>,
    /// How far each sample's place on the grid is off the face.
    gaps: Vec<f64>,
    /// Which faces its samples can fall in the same places as.
    sample_grid: SampleGrid,
    /// Its corners.
    corners: Vec<Point>,
    /// Where its grid sits.
    frame: GridFrame,
    /// Which way each sample faces, for a chart, whose surface isn't flat.
    normals: Option<Vec<Point>>,
}

/// What faces have in common when their samples fall in the same places: a plane
/// and a texture mapping.
type SampleGrid = ([i64; 4], [[u32; 4]; 2]);

/// The [`SampleGrid`] of `face`.
fn sample_grid(face: &Face, normal: Point, distance: f64) -> SampleGrid {
    #[expect(clippy::cast_possible_truncation, reason = "worlds are small")]
    let rounded = |value: f64, scale: f64| (value * scale).round() as i64;
    let plane = [
        rounded(normal[0], 256.0),
        rounded(normal[1], 256.0),
        rounded(normal[2], 256.0),
        rounded(distance, 4.0),
    ];
    // Adding nothing makes a negative zero a zero.
    let axes = face.axes.map(|axis| {
        [axis.axis.x, axis.axis.y, axis.axis.z, axis.offset].map(|part| (part + 0.0).to_bits())
    });
    (plane, axes)
}

/// A face's light samples, before any light is worked out at them.
struct LightGrid {
    /// Its shape.
    light: FaceLight,
    /// Where each sample is lit from.
    points: Vec<Point>,
    /// How far each sample's place on the grid is off the face.
    gaps: Vec<f64>,
    /// The face's corners.
    corners: Vec<Point>,
    /// Where the grid sits.
    frame: GridFrame,
}

/// `place` moved onto a convex face, just inside its edges. The grid of a
/// thin face reaches well past it, where the light isn't the face's own.
fn onto_face(corners: &[Point], normal: Point, place: Point) -> Point {
    #[expect(clippy::cast_precision_loss, reason = "faces have few corners")]
    let share = 1.0 / corners.len() as f64;
    let middle = corners
        .iter()
        .fold([0.0; 3], |sum, corner| add_scaled(sum, *corner, share));
    let mut out = place;
    let mut moved = false;
    // Twice, as moving onto one edge can leave a point past the one next to it.
    for _ in 0..2 {
        for (index, corner) in corners.iter().enumerate() {
            let next = corners[(index + 1) % corners.len()];
            let Some(mut inward) = normalized(cross(normal, add_scaled(next, *corner, -1.0)))
            else {
                continue;
            };
            if dot(inward, add_scaled(middle, *corner, -1.0)) < 0.0 {
                inward = inward.map(|part| -part);
            }
            let distance = dot(inward, add_scaled(out, *corner, -1.0));
            if distance < 0.0 {
                out = add_scaled(out, inward, -distance);
                moved = true;
            }
        }
    }
    if moved {
        out = add_scaled(out, add_scaled(middle, out, -1.0), 0.02);
    }
    out
}

/// How far `point` is from a line through `points`.
fn distance_to_line(point: Point, points: &[Point]) -> f64 {
    let mut nearest = f64::MAX;
    if let [only] = points {
        let offset = add_scaled(point, *only, -1.0);
        return dot(offset, offset).sqrt();
    }
    for pair in points.windows(2) {
        let edge = add_scaled(pair[1], pair[0], -1.0);
        let along = dot(add_scaled(point, pair[0], -1.0), edge) / dot(edge, edge).max(1e-9);
        let closest = add_scaled(pair[0], edge, along.clamp(0.0, 1.0));
        let offset = add_scaled(point, closest, -1.0);
        nearest = nearest.min(dot(offset, offset).sqrt());
    }
    nearest
}

/// How much of each layer `strokes` leave at `point`.
fn paint_at(point: Point, strokes: &[PaintStroke]) -> [u8; 4] {
    let mut weights = [0.0_f64; 4];
    for stroke in strokes {
        let share = 1.0 - distance_to_line(point, &stroke.points) / stroke.radius.max(1.0);
        let smooth = share.clamp(0.0, 1.0).powi(2) * (3.0 - 2.0 * share.clamp(0.0, 1.0));
        let weight = &mut weights[stroke.layer];
        *weight = weight.max(smooth * stroke.strength);
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to a byte"
    )]
    weights.map(|weight| (weight.clamp(0.0, 1.0) * 255.0) as u8)
}

/// How much surface a face has.
fn face_area(map: &Map, face: &Face) -> f64 {
    let first = face.first_point as usize;
    let count = face.point_count as usize;
    let Some(corners) = map.points.get(first..first + count) else {
        return 0.0;
    };
    let Some(start) = corners.first().map(|corner| wide(*corner)) else {
        return 0.0;
    };
    let mut doubled = [0.0; 3];
    for pair in corners[1..].windows(2) {
        let across = cross(
            add_scaled(wide(pair[0]), start, -1.0),
            add_scaled(wide(pair[1]), start, -1.0),
        );
        doubled = add_scaled(doubled, across, 1.0);
    }
    dot(doubled, doubled).sqrt() * 0.5
}

/// The extents of a face in texels, spaced by `spacing`.
fn light_grid(scene: &Scene<'_>, map: &Map, face: &Face, spacing: f64) -> Option<LightGrid> {
    let space = face_space(map, face)?;
    let first = face.first_point as usize;
    let points = map.points.get(first..first + face.point_count as usize)?;
    let mut low = [f64::INFINITY; 2];
    let mut high = [f64::NEG_INFINITY; 2];
    for point in points {
        for axis in 0..2 {
            let value = f64::from(face.axes[axis].project(*point));
            low[axis] = low[axis].min(value);
            high[axis] = high[axis].max(value);
        }
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "face sizes are small and whole"
    )]
    let count = |axis: usize| ((high[axis] - low[axis]) / spacing).ceil() as u32 + 2;
    #[expect(clippy::cast_possible_truncation, reason = "texels fit i32")]
    let origin = [
        (low[0] / spacing).floor() as i32 * spacing as i32,
        (low[1] / spacing).floor() as i32 * spacing as i32,
    ];
    let (width, height) = (count(0), count(1));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "small"
    )]
    let texels_per_sample = spacing as u32;
    let corners: Vec<Point> = points.iter().map(|point| wide(*point)).collect();
    let place_at = |x: f64, y: f64| {
        let s = f64::from(origin[0]) + x * spacing;
        let t = f64::from(origin[1]) + y * spacing;
        solve(
            &space.rows,
            [
                s - space.rows[0][3],
                t - space.rows[1][3],
                space.plane.distance,
            ],
        )
    };
    let start = place_at(0.0, 0.0)?;
    let frame = GridFrame {
        start,
        across: add_scaled(place_at(1.0, 0.0)?, start, -1.0),
        down: add_scaled(place_at(0.0, 1.0)?, start, -1.0),
    };
    let mut positions = Vec::with_capacity((width * height) as usize);
    let mut gaps = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let s = f64::from(origin[0]) + f64::from(x) * spacing;
            let t = f64::from(origin[1]) + f64::from(y) * spacing;
            let rhs = [
                s - space.rows[0][3],
                t - space.rows[1][3],
                space.plane.distance,
            ];
            let place = solve(&space.rows, rhs)?;
            let off = add_scaled(place, onto_face(&corners, space.plane.normal, place), -1.0);
            gaps.push(dot(off, off).sqrt());
            positions.push(sample_place(scene, &corners, space.plane.normal, place));
        }
    }
    Some(LightGrid {
        light: FaceLight {
            first_sample: 0,
            width,
            height,
            texel_origin: origin,
            texels_per_sample,
            styles: [Face::NO_STYLE; 4],
            tile: RESIDENT,
            average: [0; 3],
        },
        points: positions,
        gaps,
        corners,
        frame,
    })
}

/// The average colour of the texture of `face`.
fn albedo_of(map: &Map, face: &Face) -> [f64; 3] {
    let Some(texture) = map.textures.get(face.texture as usize) else {
        return [0.5; 3];
    };
    let Some(mip) = texture.mips.last() else {
        return [0.5; 3];
    };
    let mut total = [0.0; 3];
    let pixels = mip.rgba.as_chunks::<4>().0;
    for pixel in pixels {
        for (part, byte) in total.iter_mut().zip(pixel) {
            *part += f64::from(*byte) / 255.0;
        }
    }
    #[expect(clippy::cast_precision_loss, reason = "image sizes are small")]
    let count = pixels.len().max(1) as f64;
    total.map(|part| part / count)
}

/// What's needed to look around a sample on a face.
struct SampleSurface<'a> {
    /// The face's corners.
    corners: &'a [Point],
    /// Which way it faces.
    normal: Point,
    /// Where its samples are.
    frame: &'a GridFrame,
    /// Its samples across.
    width: usize,
    /// How far its light is lifted off it, if it is.
    lift: Option<Point>,
}

/// Places across a sample's own patch of surface, as shares of the gap to the
/// next sample. They're spread so no two share a row or a column.
const SAMPLE_PATCH: [[f64; 2]; 8] = [
    [-0.44, -0.19],
    [-0.31, 0.31],
    [-0.19, -0.44],
    [-0.06, 0.06],
    [0.06, -0.06],
    [0.19, 0.44],
    [0.31, -0.31],
    [0.44, 0.19],
];

/// How much a sample's share of unshaded light has to differ from its
/// neighbour's before a shadow's edge is taken to pass between them.
const EDGE_CHANGE: f64 = 0.04;

/// Which samples a shadow's edge passes near, going by how much of each's light
/// gets through: `seen` of it.
fn near_edges(seen: &[f64], width: usize) -> Vec<bool> {
    let mut out = vec![false; seen.len()];
    for (at, value) in seen.iter().enumerate() {
        let across = at % width;
        let mut mark = |other: usize| {
            if (seen[other] - value).abs() > EDGE_CHANGE {
                out[at] = true;
                out[other] = true;
            }
        };
        if across + 1 < width {
            mark(at + 1);
        }
        if at + width < seen.len() {
            mark(at + width);
        }
    }
    out
}

/// The light on a sample's patch of surface, not only at its middle, so a thin
/// shadow comes out as how much of the patch it covers.
fn light_on_patch(
    scene: &Scene<'_>,
    source: &BakeLight,
    at: usize,
    surface: &SampleSurface<'_>,
) -> ([f64; 3], Point) {
    let (x, y) = (at % surface.width, at / surface.width);
    #[expect(clippy::cast_precision_loss, reason = "faces are small")]
    let (x, y) = (x as f64, y as f64);
    let mut color = [0.0; 3];
    let mut from = [0.0; 3];
    for [dx, dy] in SAMPLE_PATCH {
        let place = add_scaled(
            add_scaled(surface.frame.start, surface.frame.across, x + dx),
            surface.frame.down,
            y + dy,
        );
        let place = sample_place(scene, surface.corners, surface.normal, place);
        let place = surface
            .lift
            .map_or(place, |lift| add_scaled(place, lift, 1.0));
        let (light, direction) = light_on(scene, source, place, surface.normal, true);
        from = add_scaled(from, direction, light[0] + light[1] + light[2]);
        for part in 0..3 {
            color[part] += light[part] / SAMPLE_PATCH.len() as f64;
        }
    }
    (color, from)
}

/// What one light does to one face.
struct FaceShare<'a> {
    /// The light.
    source: &'a BakeLight,
    /// Its colour at each sample.
    colors: Vec<[f64; 3]>,
    /// Where it comes from at each sample, by how much there is.
    directions: Vec<Point>,
    /// How much of it there is in all, which says who keeps a layer.
    energy: f64,
}

/// How much light is in `color`.
fn energy_of(color: [f64; 3]) -> f64 {
    color[0] + color[1] + color[2]
}

/// What `source` does to the samples at `points`, or `None` if that's nothing. A
/// sharp light's shadow is left out, as its edges hold that. Anything else looks
/// across each sample's patch of surface wherever a shadow's edge is near.
fn light_of_face<'a>(
    scene: &Scene<'_>,
    source: &'a BakeLight,
    points: &[Point],
    surface: &SampleSurface<'_>,
) -> Option<FaceShare<'a>> {
    let normal = surface.normal;
    let open: Vec<([f64; 3], Point)> = points
        .iter()
        .map(|point| light_on(scene, source, *point, normal, false))
        .collect();
    let most = open.iter().map(|(color, _)| energy_of(*color)).sum::<f64>();
    if most <= 0.0 {
        return None;
    }
    let mut lit = open.clone();
    if !source.crisp {
        lit = points
            .iter()
            .map(|point| light_on(scene, source, *point, normal, true))
            .collect();
        let seen: Vec<f64> = lit
            .iter()
            .zip(&open)
            .map(|((color, _), (whole, _))| energy_of(*color) / energy_of(*whole).max(1e-9))
            .collect();
        for (at, near) in near_edges(&seen, surface.width).iter().enumerate() {
            if *near && energy_of(open[at].0) > 0.0 {
                lit[at] = light_on_patch(scene, source, at, surface);
            }
        }
    }
    let mut share = FaceShare {
        source,
        colors: Vec::with_capacity(lit.len()),
        directions: Vec::with_capacity(lit.len()),
        energy: 0.0,
    };
    for (color, direction) in lit {
        let energy = energy_of(color);
        share.energy += energy;
        share.colors.push(color);
        share.directions.push(direction.map(|part| part * energy));
    }
    (share.energy > 0.0).then_some(share)
}

/// A light layer, and the sharp light it holds, if it's one.
type FitLayer<'a> = (u8, Vec<[f64; 3]>, Option<&'a BakeLight>);

/// What fits on a face, what's left over from `lights`, in the order of how much
/// light each gives, and so nothing depends on the order they're in a map. One
/// layer is for bounced light.
fn rank_layers<'a>(mut lights: Vec<FaceShare<'a>>) -> (Vec<FitLayer<'a>>, Vec<FaceShare<'a>>) {
    lights.sort_by(|a, b| b.energy.total_cmp(&a.energy));
    let room = MAX_LAYERS - 1;
    let mut layers: Vec<FitLayer<'a>> = Vec::new();
    let mut spare_crisp = CRISP_STYLES.iter().copied();
    let mut left = Vec::new();
    for share in lights {
        let held = layers.iter().position(|(style, _, sharp)| {
            sharp.is_none() && !share.source.crisp && *style == share.source.style
        });
        if let Some(at) = held {
            for (sum, color) in layers[at].1.iter_mut().zip(&share.colors) {
                *sum = std::array::from_fn(|part| sum[part] + color[part]);
            }
            continue;
        }
        if layers.len() >= room {
            left.push(share);
            continue;
        }
        if share.source.crisp {
            match spare_crisp.next() {
                Some(style) => layers.push((style, share.colors, Some(share.source))),
                None => left.push(share),
            }
        } else {
            layers.push((share.source.style, share.colors, None));
        }
    }
    (layers, left)
}

/// A face's layers, where its light comes from, and its sharp lights by style.
type FittedLayers<'a> = (
    Vec<(u8, Vec<[f64; 3]>)>,
    Vec<Point>,
    Vec<(u8, &'a BakeLight)>,
);

/// Fits `lights` to the layers a face has. What doesn't get a layer of its own
/// isn't lost: it joins the plainest one, as light that doesn't change. Returns
/// the layers, where light comes from, and the sharp lights with their styles.
fn fit_layers<'a>(
    scene: &Scene<'_>,
    lights: Vec<FaceShare<'a>>,
    points: &[Point],
    normal: Point,
) -> FittedLayers<'a> {
    let mut directions = vec![[0.0; 3]; points.len()];
    for share in &lights {
        for (sum, direction) in directions.iter_mut().zip(&share.directions) {
            *sum = add_scaled(*sum, *direction, 1.0);
        }
    }
    let (mut layers, left) = rank_layers(lights);
    if !left.is_empty() && layers.iter().all(|(_, _, sharp)| sharp.is_some()) {
        // Light left over needs somewhere plain to go, so the weakest sharp
        // light gives way. Its samples then have to hold its shadow.
        if let Some((_, _, Some(source))) = layers.pop() {
            let shaded = points
                .iter()
                .map(|point| light_on(scene, source, *point, normal, true).0)
                .collect();
            layers.push((source.style, shaded, None));
        }
    }
    for share in left {
        let colors: Vec<[f64; 3]> = if share.source.crisp {
            points
                .iter()
                .map(|point| light_on(scene, share.source, *point, normal, true).0)
                .collect()
        } else {
            share.colors
        };
        let plain = layers
            .iter_mut()
            .filter(|(_, _, sharp)| sharp.is_none())
            .min_by_key(|(style, _, _)| *style);
        if let Some((_, target, _)) = plain {
            for (sum, color) in target.iter_mut().zip(&colors) {
                *sum = std::array::from_fn(|part| sum[part] + color[part]);
            }
        }
    }
    let sharp = layers
        .iter()
        .filter_map(|(style, _, sharp)| sharp.map(|source| (*style, source)))
        .collect();
    let layers = layers
        .into_iter()
        .map(|(style, colors, _)| (style, colors))
        .collect();
    (layers, directions, sharp)
}

/// Lights one face.
fn bake_face(scene: &Scene<'_>, map: &Map, face: &Face, spacing: f64) -> Option<FaceBake> {
    let texture = map.textures.get(face.texture as usize)?;
    if !matches!(texture.kind, TextureKind::Solid | TextureKind::Cutout) {
        return None;
    }
    let space = face_space(map, face)?;
    let normal = space.plane.normal;
    let LightGrid {
        light,
        points,
        gaps,
        corners,
        frame: grid,
    } = light_grid(scene, map, face, spacing)?;
    let shine_at = scene.lifts.get(&face.first_point).map(|lift| wide(*lift));
    let lifted: Vec<Point> = points
        .iter()
        .map(|point| shine_at.map_or(*point, |lift| add_scaled(*point, lift, 1.0)))
        .collect();
    let surface = SampleSurface {
        corners: &corners,
        normal,
        frame: &grid,
        width: light.width as usize,
        lift: shine_at,
    };
    let mut lights = Vec::new();
    for source in scene.lights {
        if let Some(found) = light_of_face(scene, source, &lifted, &surface) {
            lights.push(found);
        }
    }
    let (layers, directions, crisp) = fit_layers(scene, lights, &lifted, normal);
    let edges = crisp
        .iter()
        .map(|(style, source)| {
            let edges = crisp_edges(scene, source, map, face, normal, &grid, &light);
            (*style, edges)
        })
        .collect();
    Some(FaceBake {
        layers,
        directions,
        light,
        points,
        normal,
        albedo: albedo_of(map, face),
        area: face_area(map, face),
        edges,
        gaps,
        sample_grid: sample_grid(face, normal, space.plane.distance),
        corners,
        frame: grid,
        normals: None,
    })
}

/// Lights what's at `point` from all round, seen from `reach` away, for something
/// lit by a probe. Returns the colour and where most of it comes from.
fn all_round(scene: &Scene<'_>, source: &BakeLight, point: Point, reach: f64) -> ([f64; 3], Point) {
    let mut color = [0.0; 3];
    let mut from = [0.0; 3];
    for side in PROBE_SIDES {
        let outside = add_scaled(point, side, reach);
        let (light, direction) = light_on(scene, source, outside, side, true);
        let energy = light[0] + light[1] + light[2];
        from = add_scaled(from, direction, energy);
        for part in 0..3 {
            color[part] += light[part] / PROBE_SIDES.len() as f64 * 1.5;
        }
    }
    (color, from)
}

/// Lights one chart of a mesh, which is `number` of them. Each sample takes the
/// light that falls on the surface there, by its own normal, so the mesh looks
/// smooth, and no light is sharp, as one sample has to serve a curve.
fn bake_chart(
    scene: &Scene<'_>,
    map: &Map,
    chart: &Chart,
    number: usize,
    spacing: f64,
) -> Option<FaceBake> {
    let first = map.faces.get(*chart.faces.first()? as usize)?;
    let count = chart.spots.len();
    let mut layers: Vec<(u8, Vec<[f64; 3]>)> = Vec::new();
    let mut directions = vec![[0.0; 3]; count];
    for source in scene.lights {
        let at = match layers.iter().position(|(known, _)| *known == source.style) {
            Some(at) => at,
            None if layers.len() + 1 < MAX_LAYERS => {
                layers.push((source.style, vec![[0.0; 3]; count]));
                layers.len() - 1
            }
            None => continue,
        };
        for (index, spot) in chart.spots.iter().enumerate() {
            let (color, direction) = match chart.probe_reach {
                Some(reach) => all_round(scene, source, spot.point, reach),
                None => light_on(scene, source, spot.point, spot.normal, true),
            };
            let energy = color[0] + color[1] + color[2];
            if energy > 0.0 {
                let sample = &mut layers[at].1[index];
                for part in 0..3 {
                    sample[part] += color[part];
                }
                directions[index] = add_scaled(directions[index], direction, energy);
            }
        }
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "small"
    )]
    let texels_per_sample = spacing as u32;
    #[expect(clippy::cast_possible_wrap, reason = "there aren't that many charts")]
    let own_grid = number as i64;
    Some(FaceBake {
        layers,
        directions,
        light: FaceLight {
            first_sample: 0,
            width: chart.width,
            height: chart.height,
            texel_origin: chart.origin,
            texels_per_sample,
            styles: [Face::NO_STYLE; 4],
            tile: RESIDENT,
            average: [0; 3],
        },
        // What bounces reaches a probe's mesh from above it, not from inside it.
        points: chart
            .spots
            .iter()
            .map(|spot| add_scaled(spot.point, spot.normal, chart.probe_reach.unwrap_or(0.0)))
            .collect(),
        normal: chart.normal,
        albedo: albedo_of(map, first),
        area: chart.area,
        edges: Vec::new(),
        gaps: vec![0.0; count],
        // Nothing else shares a chart's samples.
        sample_grid: ([i64::MIN, own_grid, 0, 0], [[0; 4]; 2]),
        corners: Vec::new(),
        frame: GridFrame {
            start: [0.0; 3],
            across: [0.0; 3],
            down: [0.0; 3],
        },
        normals: Some(chart.spots.iter().map(|spot| spot.normal).collect()),
    })
}

/// How far apart two edges can be and still be one join.
const JOIN_SLACK: f64 = 0.05;

/// A sample, and which face has it.
#[derive(Clone, Copy)]
struct HeldSample {
    /// Where it is on the grid.
    texel: [i64; 2],
    /// How far that is off its face.
    gap: f64,
    /// Which face has it.
    face: usize,
    /// Which of the face's samples it is.
    index: usize,
}

/// Each sample of `bake`, which is face number `face`.
fn held_samples(bake: &FaceBake, face: usize) -> impl Iterator<Item = HeldSample> {
    let light = bake.light;
    let width = light.width;
    let step = i64::from(light.texels_per_sample);
    (0_u32..).zip(&bake.gaps).map(move |(index, gap)| {
        let (x, y) = (i64::from(index % width), i64::from(index / width));
        HeldSample {
            texel: [
                i64::from(light.texel_origin[0]) + x * step,
                i64::from(light.texel_origin[1]) + y * step,
            ],
            gap: *gap,
            face,
            index: index as usize,
        }
    })
}

/// Whether two faces meet along an edge within `reach` of `place`.
fn join_near(one: &[Point], other: &[Point], place: Point, reach: f64) -> bool {
    let sides = |corners: &[Point]| -> Vec<[Point; 2]> {
        let next = corners.iter().cycle().skip(1);
        corners.iter().zip(next).map(|(a, b)| [*a, *b]).collect()
    };
    let (ours, theirs) = (sides(one), sides(other));
    ours.iter().any(|side| {
        let Some(along) = normalized(add_scaled(side[1], side[0], -1.0)) else {
            return false;
        };
        let distance_along = |point: Point| dot(add_scaled(point, side[0], -1.0), along);
        let off_line = |point: Point| {
            let closest = add_scaled(side[0], along, distance_along(point));
            let offset = add_scaled(point, closest, -1.0);
            dot(offset, offset).sqrt()
        };
        theirs.iter().any(|across| {
            if off_line(across[0]) > JOIN_SLACK || off_line(across[1]) > JOIN_SLACK {
                return false;
            }
            let (a, b) = (distance_along(across[0]), distance_along(across[1]));
            let low = a.min(b).max(0.0);
            let high = a.max(b).min(distance_along(side[1]));
            let shared = [
                add_scaled(side[0], along, low),
                add_scaled(side[0], along, high),
            ];
            high - low > JOIN_SLACK && distance_to_line(place, &shared) < reach
        })
    })
}

/// Which sample each of `faces`, all on one grid, takes its light from. Where
/// samples fall in one place, the one on its own face knows that place best. Two
/// are joined if they're in sight of each other, as a wall between them means the
/// light on its far side isn't theirs, or if their faces meet at that place, as
/// then they have to show the same light up to their join.
fn shared_samples(
    scene: &Scene<'_>,
    bakes: &[Option<FaceBake>],
    faces: &[usize],
) -> Vec<(HeldSample, HeldSample)> {
    let mut held: Vec<HeldSample> = faces
        .iter()
        .filter_map(|face| Some(held_samples(bakes[*face].as_ref()?, *face)))
        .flatten()
        .collect();
    held.sort_by(|a, b| {
        a.texel
            .cmp(&b.texel)
            .then(a.gap.total_cmp(&b.gap))
            .then(a.face.cmp(&b.face))
    });
    let lit_from = |sample: &HeldSample| {
        let bake = bakes[sample.face].as_ref()?;
        Some(add_scaled(
            bake.points[sample.index],
            bake.normal,
            SURFACE_OFFSET,
        ))
    };
    let in_sight = |a: &HeldSample, b: &HeldSample| {
        let ends = lit_from(a).zip(lit_from(b));
        ends.is_some_and(|(a, b)| !blocked(scene, scene.root, a, b))
    };
    let meet = |a: &HeldSample, b: &HeldSample| {
        let (Some(one), Some(other)) = (&bakes[a.face], &bakes[b.face]) else {
            return false;
        };
        let frame = &one.frame;
        #[expect(clippy::cast_precision_loss, reason = "faces are small")]
        let place = add_scaled(
            add_scaled(
                frame.start,
                frame.across,
                (a.index % one.light.width as usize) as f64,
            ),
            frame.down,
            (a.index / one.light.width as usize) as f64,
        );
        let cell = add_scaled(frame.across, frame.down, 1.0);
        a.face != b.face && join_near(&one.corners, &other.corners, place, dot(cell, cell).sqrt())
    };
    let mut shared = Vec::new();
    for place in held.chunk_by(|a, b| a.texel == b.texel) {
        // Whatever's joined, even through another, ends up with the one owner, or
        // two faces that meet could take after different samples.
        let mut owners: Vec<usize> = (0..place.len()).collect();
        for (rank, sample) in place.iter().enumerate() {
            for (other, best) in place[..rank].iter().enumerate() {
                let apart = owners[rank] != owners[other];
                if apart && (in_sight(sample, best) || meet(sample, best)) {
                    let keep = owners[rank].min(owners[other]);
                    let drop = owners[rank].max(owners[other]);
                    for owner in owners.iter_mut().filter(|owner| **owner == drop) {
                        *owner = keep;
                    }
                }
            }
        }
        for (rank, (sample, owner)) in place.iter().zip(&owners).enumerate() {
            if *owner != rank {
                shared.push((place[*owner], *sample));
            }
        }
    }
    shared
}

/// Gives `to` everything `from` has.
fn copy_sample(bakes: &mut [Option<FaceBake>], from: HeldSample, to: HeldSample) {
    let Some(source) = &bakes[from.face] else {
        return;
    };
    let layers: Vec<[f64; 3]> = source
        .layers
        .iter()
        .map(|(_, layer)| layer[from.index])
        .collect();
    let direction = source.directions[from.index];
    let edges: Vec<(u8, u8)> = source
        .edges
        .iter()
        .map(|(style, edges)| (*style, edges[from.index]))
        .collect();
    let Some(target) = &mut bakes[to.face] else {
        return;
    };
    for ((_, layer), color) in target.layers.iter_mut().zip(layers) {
        layer[to.index] = color;
    }
    target.directions[to.index] = direction;
    for (style, edge) in edges {
        let same = target.edges.iter_mut().find(|(known, _)| *known == style);
        if let Some((_, held)) = same {
            held[to.index] = edge;
        }
    }
}

/// Makes faces agree on the samples they share. Light blends between samples, so
/// two faces with the same samples either side of a join show the same light along
/// it, and a shadow runs straight on across.
fn share_samples(scene: &Scene<'_>, bakes: &mut [Option<FaceBake>]) {
    let mut grids: HashMap<SampleGrid, Vec<usize>> = HashMap::new();
    for (number, bake) in bakes.iter().enumerate() {
        if let Some(bake) = bake {
            grids.entry(bake.sample_grid).or_default().push(number);
        }
    }
    let mut groups: Vec<Vec<usize>> = grids
        .into_values()
        .filter(|faces| faces.len() > 1)
        .collect();
    groups.sort_unstable();
    let known: &[Option<FaceBake>] = bakes;
    let shared = in_parallel(&groups, |faces| shared_samples(scene, known, faces));
    for (from, to) in shared.into_iter().flatten() {
        copy_sample(bakes, from, to);
    }
}

/// Where a sample at `place` on a face is lit from. Samples are on a grid that
/// runs past the face's edges, so that light can blend up to them. Most of those
/// lie over the next face along, and have to be lit where they are, or two faces
/// disagree where they join and a shadow steps sideways there. Only what's behind
/// something, as a wall, comes back onto the face.
fn sample_place(scene: &Scene<'_>, corners: &[Point], normal: Point, place: Point) -> Point {
    let on_face = onto_face(corners, normal, place);
    let lifted = |point: Point| add_scaled(point, normal, SURFACE_OFFSET);
    if blocked(scene, scene.root, lifted(on_face), lifted(place)) {
        on_face
    } else {
        place
    }
}

/// Where a light sample grid sits in the world.
#[derive(Clone, Copy, Debug)]
struct GridFrame {
    /// Its first sample.
    start: Point,
    /// From one sample to the next along a row.
    across: Point,
    /// From one row to the next.
    down: Point,
}

/// How far each sample of a face is from the edge of `source`'s shadow: 128 at
/// the edge, more when lit. Looking at a finer grid than the samples, this keeps
/// where an edge falls, which a blur of samples loses.
fn crisp_edges(
    scene: &Scene<'_>,
    source: &BakeLight,
    map: &Map,
    face: &Face,
    normal: Point,
    grid: &GridFrame,
    light: &FaceLight,
) -> Vec<u8> {
    let first = face.first_point as usize;
    let corners: Vec<Point> = map.points[first..first + face.point_count as usize]
        .iter()
        .map(|point| wide(*point))
        .collect();
    let size = |value: u32| i32::try_from(value).unwrap_or(0);
    let (width, height) = (size(light.width), size(light.height));
    let (fine_width, fine_height) = (width * EDGE_DETAIL, height * EDGE_DETAIL);
    let count = |value: i32| usize::try_from(value).unwrap_or(0);
    let scale = 1.0 / f64::from(EDGE_DETAIL);
    let visible = |x: i32, y: i32| {
        let along = (f64::from(x) + 0.5) * scale - 0.5;
        let across = (f64::from(y) + 0.5) * scale - 0.5;
        let place = add_scaled(
            add_scaled(grid.start, grid.across, along),
            grid.down,
            across,
        );
        let place = sample_place(scene, &corners, normal, place);
        let (color, _) = light_on(scene, source, place, normal, true);
        color[0] + color[1] + color[2] > 0.0
    };
    let mut seen = Vec::with_capacity(count(fine_width * fine_height));
    for y in 0..fine_height {
        for x in 0..fine_width {
            seen.push(visible(x, y));
        }
    }
    let at = |x: i32, y: i32| {
        let x = x.clamp(0, fine_width - 1);
        let y = y.clamp(0, fine_height - 1);
        seen[count(y * fine_width + x)]
    };
    let mut edges = Vec::with_capacity(count(width * height));
    for y in 0..height {
        for x in 0..width {
            let (middle_x, middle_y) = (
                x * EDGE_DETAIL + EDGE_DETAIL / 2,
                y * EDGE_DETAIL + EDGE_DETAIL / 2,
            );
            let lit = at(middle_x, middle_y);
            let mut nearest = f64::from(EDGE_REACH);
            for dy in -EDGE_REACH..=EDGE_REACH {
                for dx in -EDGE_REACH..=EDGE_REACH {
                    if at(middle_x + dx, middle_y + dy) != lit {
                        nearest = nearest.min(f64::from(dx * dx + dy * dy).sqrt());
                    }
                }
            }
            let share = nearest / f64::from(EDGE_REACH) * 127.0;
            let signed = if lit { 128.0 + share } else { 128.0 - share };
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "between 1 and 255"
            )]
            edges.push(signed as u8);
        }
    }
    edges
}

/// A patch of lit surface that bounces light on.
#[derive(Clone, Copy)]
struct BounceSource {
    /// Where it is.
    point: Point,
    /// Which way it faces.
    normal: Point,
    /// What it sends out.
    color: [f64; 3],
}

/// What bounces light, by tile, so a point looks at what's near it however big
/// the map is. Without tiles it's all in one.
struct BounceField {
    /// How wide a tile is, or 0 for no tiles.
    tile_size: f64,
    /// What's in each tile.
    tiles: HashMap<[i64; 2], Vec<BounceSource>>,
}

impl BounceField {
    /// The tile `point` is in.
    #[expect(clippy::cast_possible_truncation, reason = "grid places are small")]
    fn tile_of(&self, point: Point) -> [i64; 2] {
        if self.tile_size <= 0.0 {
            return [0, 0];
        }
        [
            (point[0] / self.tile_size).floor() as i64,
            (point[1] / self.tile_size).floor() as i64,
        ]
    }

    /// What's near `point`: its own tile and those round it.
    fn near(&self, point: Point) -> impl Iterator<Item = &BounceSource> {
        let [x, y] = self.tile_of(point);
        let reach = i64::from(self.tile_size > 0.0);
        (-reach..=reach)
            .flat_map(move |dy| (-reach..=reach).map(move |dx| [x + dx, y + dy]))
            .filter_map(|tile| self.tiles.get(&tile))
            .flatten()
    }
}

/// Keeps `most` of `sources`, evenly, each standing for those left out.
fn thinned(sources: Vec<BounceSource>, most: usize) -> Vec<BounceSource> {
    let step = (sources.len() / most).max(1);
    #[expect(clippy::cast_precision_loss, reason = "counts are small")]
    let weight = step as f64;
    let mut picked: Vec<BounceSource> = sources.into_iter().step_by(step).collect();
    for source in &mut picked {
        source.color = source.color.map(|part| part * weight);
    }
    picked
}

/// Picks evenly from what's lit, tile by tile.
fn bounce_sources(bakes: &[Option<FaceBake>], spacing: f64, tile_size: f64) -> BounceField {
    let mut field = BounceField {
        tile_size,
        tiles: HashMap::new(),
    };
    let mut sources = Vec::new();
    for bake in bakes.iter().flatten() {
        for (index, point) in bake.points.iter().enumerate() {
            let mut lit = [0.0; 3];
            for (style, samples) in &bake.layers {
                // A crisp layer holds light as if nothing shaded it.
                let edges = bake.edges.iter().find(|(known, _)| known == style);
                let edge = edges.and_then(|(_, edges)| edges.get(index));
                if edge.is_some_and(|edge| *edge < 128) {
                    continue;
                }
                for part in 0..3 {
                    lit[part] += samples[index][part];
                }
            }
            if lit[0] + lit[1] + lit[2] > 1.0 {
                // A sample sends out only its share of the face: a sliver has
                // more samples than its area is worth.
                #[expect(clippy::cast_precision_loss, reason = "counts are small")]
                let share = bake.area / bake.points.len() as f64;
                let area = share.min(spacing * spacing);
                sources.push(BounceSource {
                    point: *point,
                    normal: bake.normal,
                    color: [
                        lit[0] * bake.albedo[0] * area,
                        lit[1] * bake.albedo[1] * area,
                        lit[2] * bake.albedo[2] * area,
                    ],
                });
            }
        }
    }
    let mut tiles: HashMap<[i64; 2], Vec<BounceSource>> = HashMap::new();
    for source in sources {
        tiles
            .entry(field.tile_of(source.point))
            .or_default()
            .push(source);
    }
    let most = if tile_size > 0.0 {
        MAX_TILE_BOUNCE_SOURCES
    } else {
        MAX_BOUNCE_SOURCES
    };
    field.tiles = tiles
        .into_iter()
        .map(|(tile, sources)| (tile, thinned(sources, most)))
        .collect();
    field
}

/// The light bounced onto a point.
fn bounced_at(
    scene: &Scene<'_>,
    sources: &BounceField,
    point: Point,
    normal: Point,
    gain: f64,
) -> [f64; 3] {
    let mut total = [0.0; 3];
    let start = add_scaled(point, normal, SURFACE_OFFSET);
    for source in sources.near(point) {
        let offset = [
            source.point[0] - start[0],
            source.point[1] - start[1],
            source.point[2] - start[2],
        ];
        let squared = dot(offset, offset).max(256.0);
        let Some(direction) = normalized(offset) else {
            continue;
        };
        let here = dot(direction, normal);
        let there = -dot(direction, source.normal);
        if here <= 0.0 || there <= 0.0 {
            continue;
        }
        let end = add_scaled(source.point, source.normal, SURFACE_OFFSET);
        if shadowed(scene, start, end) {
            continue;
        }
        let share = here * there / squared * gain;
        for (part, color) in total.iter_mut().zip(source.color) {
            *part += color * share;
        }
    }
    total
}

/// The places along a row of `count` samples where bounced light is worked out:
/// every so often, and the two at each end. A row's end hangs off its face and
/// is often in a wall, and drawing the next one from that would darken corners.
fn bounce_stops(count: usize) -> Vec<usize> {
    let inside = 1..count.saturating_sub(1);
    let mut stops: Vec<usize> = std::iter::once(0)
        .chain(inside.step_by(BOUNCE_STRIDE))
        .collect();
    for end in [count.saturating_sub(2), count.saturating_sub(1)] {
        if stops.last().is_some_and(|last| *last < end) {
            stops.push(end);
        }
    }
    stops
}

/// The stop before `at`, and how far along it is to the next.
fn between_stops(stops: &[usize], at: usize) -> (usize, f64) {
    let index = stops.partition_point(|stop| *stop <= at).saturating_sub(1);
    let index = index.min(stops.len().saturating_sub(2));
    let (Some(low), Some(high)) = (stops.get(index), stops.get(index + 1)) else {
        return (0, 0.0);
    };
    #[expect(clippy::cast_precision_loss, reason = "faces are small")]
    let share = (at - low) as f64 / (high - low) as f64;
    (index, share)
}

/// The light bounced onto each sample of a face.
fn bounced_face(
    scene: &Scene<'_>,
    sources: &BounceField,
    bake: &FaceBake,
    gain: f64,
) -> Vec<[f64; 3]> {
    let (width, height) = (bake.light.width as usize, bake.light.height as usize);
    let columns = bounce_stops(width);
    let rows = bounce_stops(height);
    let mut known = Vec::with_capacity(columns.len() * rows.len());
    for row in &rows {
        for column in &columns {
            let at = row * width + column;
            let normal = bake.normals.as_ref().map_or(bake.normal, |all| all[at]);
            known.push(bounced_at(scene, sources, bake.points[at], normal, gain));
        }
    }
    let read = |column: usize, row: usize| {
        known[row.min(rows.len() - 1) * columns.len() + column.min(columns.len() - 1)]
    };
    let mut out = Vec::with_capacity(bake.points.len());
    for y in 0..height {
        let (row, down) = between_stops(&rows, y);
        for x in 0..width {
            let (column, across) = between_stops(&columns, x);
            let mut color = [0.0; 3];
            for (part, value) in color.iter_mut().enumerate() {
                let top =
                    read(column, row)[part] * (1.0 - across) + read(column + 1, row)[part] * across;
                let bottom = read(column, row + 1)[part] * (1.0 - across)
                    + read(column + 1, row + 1)[part] * across;
                *value = top * (1.0 - down) + bottom * down;
            }
            out.push(color);
        }
    }
    out
}

/// A colour on the lightmap's scale, from 0 to 255.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to a byte"
)]
fn to_bytes(color: [f64; 3]) -> [u8; 3] {
    // Too bright for a byte, the whole colour comes down together. Cutting off
    // each part alone turns orange light green, as red stops and green doesn't.
    let brightest = color.iter().copied().fold(0.0, f64::max);
    let share = if brightest > 255.0 {
        255.0 / brightest
    } else {
        1.0
    };
    color.map(|part| (part * share).clamp(0.0, 255.0) as u8)
}

/// A direction as bytes.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamped to a byte"
)]
fn direction_bytes(direction: Point) -> [u8; 3] {
    let unit = normalized(direction).unwrap_or([0.0, 0.0, 0.0]);
    unit.map(|part| ((part * 0.5 + 0.5) * 255.0).clamp(0.0, 255.0) as u8)
}

/// Runs `work` over `items` on every core, in order.
pub(crate) fn in_parallel<T: Sync, R: Send>(items: &[T], work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    // Each thread takes the next few as it's free. Handing each a fixed share
    // leaves the rest waiting on whichever got the part of a map with the most
    // going on.
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, Vec<R>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads.max(1))
            .map(|_| {
                scope.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let first = next.fetch_add(PARALLEL_BATCH, Ordering::Relaxed);
                        let Some(batch) = items.get(first..) else {
                            return mine;
                        };
                        if batch.is_empty() {
                            return mine;
                        }
                        let batch = &batch[..batch.len().min(PARALLEL_BATCH)];
                        mine.push((first, batch.iter().map(&work).collect::<Vec<R>>()));
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| {
                handle
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    done.sort_by_key(|(first, _)| *first);
    done.into_iter().flat_map(|(_, batch)| batch).collect()
}

/// Bakes `probes` over the world's bounds, for moving things.
#[expect(clippy::cast_possible_truncation, reason = "the tree takes f32")]
fn bake_probes(
    scene: &Scene<'_>,
    map: &Map,
    sources: &BounceField,
    gain: f64,
    ambient: f64,
) -> Option<ProbeGrid> {
    let model = map.models.first()?;
    let (low, high) = (wide(model.mins), wide(model.maxs));
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "world sizes are small"
    )]
    let count = |axis: usize| ((high[axis] - low[axis]) / PROBE_SPACING).ceil() as u32 + 1;
    let size = [count(0), count(1), count(2)];
    let mut positions = Vec::new();
    for z in 0..size[2] {
        for y in 0..size[1] {
            for x in 0..size[0] {
                positions.push([
                    low[0] + f64::from(x) * PROBE_SPACING,
                    low[1] + f64::from(y) * PROBE_SPACING,
                    low[2] + f64::from(z) * PROBE_SPACING,
                ]);
            }
        }
    }
    let baked = in_parallel(&positions, |point| {
        let mut colors = [[0_u8; 3]; 6];
        let mut direction = [0.0; 3];
        // A probe in solid stays dark, which is how sampling knows to skip it.
        let place = Vec3::new(point[0] as f32, point[1] as f32, point[2] as f32);
        let leaf = map.tree.leaf(map.tree.leaf_at(place));
        let open = leaf.is_some_and(|leaf| leaf.contents != engine_map::Contents::Solid);
        let base = if open { ambient } else { 0.0 };
        for (side, normal) in PROBE_SIDES.iter().enumerate() {
            let mut light = bounced_at(scene, sources, *point, *normal, gain);
            for source in scene.lights {
                let (color, from) = light_on(scene, source, *point, *normal, true);
                let energy = color[0] + color[1] + color[2];
                direction = add_scaled(direction, from, energy);
                for part in 0..3 {
                    light[part] += color[part];
                }
            }
            colors[side] = to_bytes(light.map(|part| part + base));
        }
        (colors, direction_bytes(direction))
    });
    #[expect(clippy::cast_possible_truncation, reason = "stored as f32")]
    let origin = Vec3::new(low[0] as f32, low[1] as f32, low[2] as f32);
    #[expect(clippy::cast_possible_truncation, reason = "stored as f32")]
    let spacing = PROBE_SPACING as f32;
    Some(ProbeGrid {
        origin,
        spacing,
        size,
        colors: baked.iter().map(|(colors, _)| *colors).collect(),
        directions: baked.iter().map(|(_, direction)| *direction).collect(),
    })
}

/// What a bake needs beyond the map.
pub struct BakeInput<'a> {
    /// The map as written.
    pub working: &'a WorkingMap,
    /// Reads an image by path.
    pub images: &'a mut ImageReader<'a>,
}

/// The material of `face`, if it has one.
fn material_of<'a>(map: &'a Map, face: &Face) -> Option<&'a Material> {
    map.textures.get(face.texture as usize)?.material.as_deref()
}

/// Bakes the light of `map`.
///
/// # Errors
///
/// Returns an error if a light's image can't be read.
pub fn bake_light(map: &mut Map, input: &mut BakeInput<'_>) -> Result<(), CompileError> {
    let working = input.working;
    let mut lights = read_lights(working, input.images)?;
    let world = working
        .entities
        .iter()
        .find(|entity| entity.kind == "worldspawn");
    let spacing = world
        .and_then(|world| world.number("light_spacing"))
        .unwrap_or(DEFAULT_SPACING)
        .max(1.0);
    let ambient = world
        .and_then(|world| world.number("ambient"))
        .unwrap_or(0.0);
    let bounce_gain = world
        .and_then(|world| world.number("bounce"))
        .unwrap_or(1.0)
        * 0.25;
    if let Some(sky) = &map.sky {
        let direction = sky.sun_direction;
        let intensity = world
            .and_then(|world| world.number("sun_intensity"))
            .unwrap_or(0.0);
        if intensity > 0.0 {
            lights.push(BakeLight {
                origin: [0.0; 3],
                color: sky.sun_color.map(f64::from),
                intensity,
                style: 0,
                size: world.and_then(|w| w.number("sun_size")).unwrap_or(0.0),
                direction: Some(wide(direction)),
                cone: -1.0,
                cookie: None,
                profile: Vec::new(),
                sun: true,
                crisp: world.is_some_and(|w| {
                    w.fields
                        .iter()
                        .any(|(k, v)| k == "sun_crisp" && v.flag() == Some(true))
                }),
                angle_scale: 1.0,
            });
        }
    }

    // Light passes what's see-through, so those brushes aren't in its tree.
    let see_through = |name: &str| {
        map.textures
            .iter()
            .find(|texture| texture.name == name)
            .and_then(|texture| texture.material.as_ref())
            .is_some_and(|material| material.alpha == AlphaMode::Blend)
    };
    let solid: Vec<_> = working
        .brushes
        .iter()
        .filter(|brush| {
            brush.contents == engine_map::Contents::Solid
                && !brush.faces.iter().any(|face| see_through(&face.material))
        })
        .cloned()
        .collect();
    let mut planes = PlaneSet::default();
    let (tree, root) = crate::build::build_tree(&solid, None, &mut planes);
    let mut glass = Vec::new();
    let model_faces = map.models.first().map_or(0, |model| model.face_count) as usize;
    for face in map.faces.iter().take(model_faces) {
        let Some(material) = material_of(map, face) else {
            continue;
        };
        if material.alpha != AlphaMode::Blend {
            continue;
        }
        let image = material
            .tint
            .and_then(|slot| material.slots.get(usize::from(slot).saturating_sub(1)))
            .cloned();
        let (Some(image), Some(space)) = (image, face_space(map, face)) else {
            continue;
        };
        let first = face.first_point as usize;
        let Some(points) = map.points.get(first..first + face.point_count as usize) else {
            continue;
        };
        glass.push(Glass {
            plane: space.plane,
            points: points.iter().map(|point| wide(*point)).collect(),
            axes: [
                (
                    [space.rows[0][0], space.rows[0][1], space.rows[0][2]],
                    space.rows[0][3],
                ),
                (
                    [space.rows[1][0], space.rows[1][1], space.rows[1][2]],
                    space.rows[1][3],
                ),
            ],
            image,
        });
    }
    let meshes: Vec<MeshShadow> = working
        .placed
        .iter()
        .filter(|mesh| !mesh.only_seen)
        .filter_map(MeshShadow::of)
        .collect();
    let scene = Scene {
        tree,
        root,
        planes,
        glass_grid: RayGrid::new(glass.iter().map(Glass::bounds)),
        glass,
        mesh_grid: RayGrid::new(meshes.iter().map(|mesh| (mesh.low, mesh.high))),
        meshes,
        top: map
            .models
            .first()
            .map_or(0.0, |world| f64::from(world.maxs.z)),
        lifts: map
            .light_lifts
            .iter()
            .filter_map(|(face, lift)| Some((map.faces.get(*face as usize)?.first_point, *lift)))
            .collect(),
        lights: &lights,
    };

    let faces: Vec<Face> = map.faces.clone();
    let shared: &Map = map;
    let charts = map_charts(shared, spacing);
    let mut charted = vec![false; faces.len()];
    for face in charts.iter().flat_map(|chart| &chart.faces) {
        charted[*face as usize] = true;
    }
    let numbered: Vec<(&Face, bool)> = faces.iter().zip(charted.iter().copied()).collect();
    let mut bakes: Vec<Option<FaceBake>> = in_parallel(&numbered, |(face, charted)| {
        (!charted)
            .then(|| bake_face(&scene, shared, face, spacing))
            .flatten()
    });
    let numbered_charts: Vec<(usize, &Chart)> = charts.iter().enumerate().collect();
    let chart_bakes = in_parallel(&numbered_charts, |(number, chart)| {
        bake_chart(&scene, shared, chart, *number, spacing)
    });
    for (chart, bake) in charts.iter().zip(chart_bakes) {
        if let Some(first) = chart.faces.first() {
            bakes[*first as usize] = bake;
        }
    }
    let sources = bounce_sources(&bakes, spacing, f64::from(shared.tile_size));
    let bounced: Vec<Option<Vec<[f64; 3]>>> = in_parallel(&bakes, |bake| {
        bake.as_ref().map(|bake| {
            let mut light = bounced_face(&scene, &sources, bake, bounce_gain);
            for color in &mut light {
                *color = color.map(|part| part + ambient);
            }
            light
        })
    });
    let probes = bake_probes(&scene, shared, &sources, bounce_gain, ambient);
    for (bake, bounce) in bakes.iter_mut().zip(bounced) {
        if let (Some(bake), Some(bounce)) = (bake, bounce) {
            bake.layers.push((BOUNCE_STYLE, bounce));
        }
    }
    share_samples(&scene, &mut bakes);

    let mut samples = Vec::new();
    let mut directions = Vec::new();
    let mut edges = Vec::new();
    let mut paint = Vec::new();
    let mut lit_faces = Vec::with_capacity(faces.len());
    for bake in bakes {
        let Some(bake) = bake else {
            lit_faces.push(None);
            continue;
        };
        let mut light = bake.light;
        light.first_sample = u32::try_from(samples.len()).unwrap_or(u32::MAX);
        for (index, (style, layer)) in bake.layers.iter().enumerate().take(MAX_LAYERS) {
            light.styles[index] = *style;
            samples.extend(layer.iter().map(|color| to_bytes(*color)));
            directions.extend(bake.directions.iter().map(|d| direction_bytes(*d)));
            match bake.edges.iter().find(|(known, _)| known == style) {
                Some((_, found)) => edges.extend_from_slice(found),
                None => edges.resize(edges.len() + layer.len(), u8::MAX),
            }
            // Paint is read with the first layer's place.
            if !working.paint.is_empty() {
                if index == 0 {
                    paint.extend(bake.points.iter().map(|p| paint_at(*p, &working.paint)));
                } else {
                    paint.resize(paint.len() + layer.len(), [0; 4]);
                }
            }
        }
        lit_faces.push(Some(light));
    }
    for (face, light) in map.faces.iter_mut().zip(lit_faces) {
        face.light = light;
    }
    for chart in &charts {
        let held = chart
            .faces
            .first()
            .and_then(|first| map.faces[*first as usize].light);
        for number in &chart.faces {
            let face = &mut map.faces[*number as usize];
            face.light = held;
            face.light_axes = Some(chart.axes);
        }
    }
    map.light = samples;
    map.light_directions = directions;
    map.light_edges = edges;
    map.paint = paint;
    let sight = |a: Vec3, b: Vec3| !blocked(&scene, scene.root, wide(a), wide(b));
    bake_visibility(map, &sight);
    map.nav = Some(build_nav(map, 0));
    map.nav_large = Some(build_nav(map, 1));
    let strokes = &working.paint;
    let mega = bake_mega(map, strokes, &|point, strokes| {
        paint_at(wide(point), strokes)
    });
    map.mega = mega.map(|(places, rgba, maps)| {
        let faces = places
            .iter()
            .flat_map(|place| [place.face.cast_signed(), place.shift[0], place.shift[1]])
            .collect();
        MegaImage { faces, rgba, maps }
    });
    map.probes = probes;
    map.lit = true;
    Ok(())
}
