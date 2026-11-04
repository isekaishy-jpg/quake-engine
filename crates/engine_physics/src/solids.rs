//! Rebuilding convex solids from a clipping hull.
//!
//! Every solid leaf of a BSP tree is a convex region, so clipping a box down
//! the tree recovers it.

use engine_core::math::Vec3;
use engine_map::{ClipChild, Plane, Tree};

/// A point in double precision, because a solid is clipped many times.
type Point = [f64; 3];

/// Points closer than this to a plane count as on it.
const ON_PLANE: f64 = 1e-6;

/// Corners closer together than this count as one.
const SAME_POINT: f64 = 1e-4;

/// How far past a model's bounds the box starts, which also covers the growth
/// of the hull.
const BOUNDS_MARGIN: f32 = 128.0;

/// The dot product.
fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The cross product.
fn cross(a: Point, b: Point) -> Point {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// `a - b`.
fn minus(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The polygons of a box.
fn box_faces(mins: Point, maxs: Point) -> Vec<Vec<Point>> {
    let corner = |x: usize, y: usize, z: usize| {
        [
            if x == 0 { mins[0] } else { maxs[0] },
            if y == 0 { mins[1] } else { maxs[1] },
            if z == 0 { mins[2] } else { maxs[2] },
        ]
    };
    let quad = |a: [usize; 3], b: [usize; 3], c: [usize; 3], d: [usize; 3]| {
        [a, b, c, d]
            .iter()
            .map(|&[x, y, z]| corner(x, y, z))
            .collect::<Vec<_>>()
    };
    vec![
        quad([0, 0, 0], [0, 1, 0], [0, 1, 1], [0, 0, 1]),
        quad([1, 0, 0], [1, 1, 0], [1, 1, 1], [1, 0, 1]),
        quad([0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]),
        quad([0, 1, 0], [1, 1, 0], [1, 1, 1], [0, 1, 1]),
        quad([0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0]),
        quad([0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]),
    ]
}

/// Keeps the part of `polygon` where `dot(normal, p) - distance >= 0`, adding
/// new corners to `cut`.
fn clip_polygon(
    polygon: &[Point],
    normal: Point,
    distance: f64,
    cut: &mut Vec<Point>,
) -> Vec<Point> {
    let mut kept = Vec::with_capacity(polygon.len() + 1);
    for (index, &point) in polygon.iter().enumerate() {
        let next = polygon[(index + 1) % polygon.len()];
        let side = dot(normal, point) - distance;
        let next_side = dot(normal, next) - distance;
        if side >= -ON_PLANE {
            kept.push(point);
        }
        if side.abs() <= ON_PLANE {
            cut.push(point);
        }
        if (side > ON_PLANE && next_side < -ON_PLANE) || (side < -ON_PLANE && next_side > ON_PLANE)
        {
            let fraction = side / (side - next_side);
            let cross_point = [
                point[0] + (next[0] - point[0]) * fraction,
                point[1] + (next[1] - point[1]) * fraction,
                point[2] + (next[2] - point[2]) * fraction,
            ];
            kept.push(cross_point);
            cut.push(cross_point);
        }
    }
    kept
}

/// Removes repeats from `points`.
fn distinct(points: Vec<Point>) -> Vec<Point> {
    let mut unique: Vec<Point> = Vec::with_capacity(points.len());
    for point in points {
        let repeated = unique.iter().any(|other| {
            let gap = minus(point, *other);
            dot(gap, gap) < SAME_POINT * SAME_POINT
        });
        if !repeated {
            unique.push(point);
        }
    }
    unique
}

/// Puts `points`, which lie on a plane, in order around their centre.
fn order_around(points: Vec<Point>, normal: Point) -> Vec<Point> {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a polygon has a handful of corners"
    )]
    let count = points.len() as f64;
    let mut centre = [0.0; 3];
    for point in &points {
        for axis in 0..3 {
            centre[axis] += point[axis] / count;
        }
    }
    let reference = if normal[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let across = cross(normal, reference);
    let along = cross(normal, across);
    let mut keyed: Vec<(f64, Point)> = points
        .into_iter()
        .map(|point| {
            let offset = minus(point, centre);
            (dot(offset, along).atan2(dot(offset, across)), point)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    keyed.into_iter().map(|(_, point)| point).collect()
}

/// Cuts a convex solid by a plane, keeping the front. Returns an empty list
/// if nothing's left.
fn clip_solid(faces: &[Vec<Point>], normal: Point, distance: f64) -> Vec<Vec<Point>> {
    let mut cut = Vec::new();
    let mut kept: Vec<Vec<Point>> = Vec::with_capacity(faces.len() + 1);
    for face in faces {
        let polygon = distinct(clip_polygon(face, normal, distance, &mut cut));
        if polygon.len() >= 3 {
            kept.push(polygon);
        }
    }
    let cap = distinct(cut);
    if cap.len() >= 3 {
        kept.push(order_around(cap, normal));
    }
    if kept.len() < 4 { Vec::new() } else { kept }
}

/// A plane's normal and distance, in double precision.
fn plane_parts(plane: &Plane) -> (Point, f64) {
    (
        [
            f64::from(plane.normal.x),
            f64::from(plane.normal.y),
            f64::from(plane.normal.z),
        ],
        f64::from(plane.distance),
    )
}

/// The corners of a solid.
#[expect(
    clippy::cast_possible_truncation,
    reason = "map coordinates fit f32, which is what they were stored as"
)]
fn corners(faces: &[Vec<Point>]) -> Vec<Vec3> {
    distinct(faces.iter().flatten().copied().collect())
        .into_iter()
        .map(|[x, y, z]| Vec3::new(x as f32, y as f32, z as f32))
        .collect()
}

/// Walks the tree, carrying what's left of the box.
fn walk(tree: &Tree, child: ClipChild, faces: &[Vec<Point>], solids: &mut Vec<Vec<Vec3>>) {
    match child {
        ClipChild::Empty => {}
        ClipChild::Solid => solids.push(corners(faces)),
        ClipChild::Node(index) => {
            let Some(node) = tree.clip_nodes.get(index as usize) else {
                return;
            };
            let Some(plane) = tree.planes.get(node.plane as usize) else {
                return;
            };
            let (normal, distance) = plane_parts(plane);
            let front = clip_solid(faces, normal, distance);
            if !front.is_empty() {
                walk(tree, node.children[0], &front, solids);
            }
            let flipped = [-normal[0], -normal[1], -normal[2]];
            let back = clip_solid(faces, flipped, -distance);
            if !back.is_empty() {
                walk(tree, node.children[1], &back, solids);
            }
        }
    }
}

/// The corners of every solid in the hull at `root`, within the bounds of
/// `mins` and `maxs`.
#[must_use]
pub fn build_solids(tree: &Tree, root: ClipChild, mins: Vec3, maxs: Vec3) -> Vec<Vec<Vec3>> {
    let low = |value: f32| f64::from(value - BOUNDS_MARGIN);
    let high = |value: f32| f64::from(value + BOUNDS_MARGIN);
    let faces = box_faces(
        [low(mins.x), low(mins.y), low(mins.z)],
        [high(maxs.x), high(maxs.y), high(maxs.z)],
    );
    let mut solids = Vec::new();
    walk(tree, root, &faces, &mut solids);
    solids
}
