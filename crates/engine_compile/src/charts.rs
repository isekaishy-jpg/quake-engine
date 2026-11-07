//! Laying a mesh out for light (proposal 0110).
//!
//! A mesh is thousands of triangles, and each having a light map of its own costs
//! too much and shows every edge. So triangles that face much the same way are
//! gathered into a chart, flattened onto a plane, and share one light map. That
//! plane is the mesh's second set of UVs, worked out here so nobody has to make
//! one. Light is then worked out at each sample with the surface's own smooth
//! normal, so a chart's edge doesn't show.

use std::collections::HashMap;

use engine_core::math::Vec3;
use engine_map::{CHUNK_CELLS, Face, Map, MeshGroup, TerrainSheet, TexelAxis, TextureKind};

use crate::geometry::{Point, add_scaled, cross, dot, normalized};

/// How far a triangle can turn from where its chart began: the cosine of 60
/// degrees. It has to stay well under 90, or a chart can fold over itself.
const CHART_TURN: f64 = 0.5;

/// The most triangles in a chart, which holds down what's checked as it grows.
const CHART_TRIANGLES: usize = 2048;

/// The most samples across a chart.
const CHART_SAMPLES: f64 = 96.0;

/// How far off a surface its samples sit, past where the surface of a low
/// polygon mesh can cut under its smooth normal.
const SAMPLE_LIFT: f64 = 1.5;

/// How far past a mesh a probe looks from.
const PROBE_CLEARANCE: f64 = 4.0;

/// One triangle of a mesh, ready to be laid out.
#[derive(Clone, Copy, Debug)]
struct Triangle {
    /// Its face.
    face: u32,
    /// Its corners.
    points: [Point; 3],
    /// Which way they face.
    normals: [Point; 3],
    /// Which way it faces, flat.
    facing: Point,
    /// How much surface it has.
    area: f64,
}

/// A triangle on a chart's plane.
type Flat = [[f64; 2]; 3];

/// A place on a surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Spot {
    /// Where it's lit from.
    pub(crate) point: Point,
    /// Which way it faces.
    pub(crate) normal: Point,
}

/// Triangles sharing a light map.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Chart {
    /// Its faces. The first holds its light.
    pub(crate) faces: Vec<u32>,
    /// How a place becomes a place on its light map.
    pub(crate) axes: [TexelAxis; 2],
    /// Samples across.
    pub(crate) width: u32,
    /// Samples down.
    pub(crate) height: u32,
    /// The texel of its first sample.
    pub(crate) origin: [i32; 2],
    /// The surface at each sample.
    pub(crate) spots: Vec<Spot>,
    /// Which way it faces, on the whole.
    pub(crate) normal: Point,
    /// How much surface it has.
    pub(crate) area: f64,
    /// For a whole mesh lit as a probe is, how far from its one sample to look
    /// from, which is past the mesh, so that it doesn't shade itself.
    pub(crate) probe_reach: Option<f64>,
}

/// A direction as a map keeps it.
fn unpacked(normal: [i8; 3]) -> Point {
    normal.map(|part| f64::from(part) / 127.0)
}

/// A point, in full.
fn wide(point: Vec3) -> Point {
    [f64::from(point.x), f64::from(point.y), f64::from(point.z)]
}

/// The triangles of `group`, which are the faces that take light.
fn triangles_of(map: &Map, group: &MeshGroup) -> Vec<Triangle> {
    let first = group.first_face as usize;
    let faces = map.faces.get(first..first + group.face_count as usize);
    (group.first_face..)
        .zip(faces.unwrap_or_default())
        .filter_map(|(number, face)| triangle_of(map, number, face))
        .collect()
}

/// The triangle of `face`, if it takes light.
fn triangle_of(map: &Map, number: u32, face: &Face) -> Option<Triangle> {
    let texture = map.textures.get(face.texture as usize)?;
    if !matches!(texture.kind, TextureKind::Solid | TextureKind::Cutout) {
        return None;
    }
    let first = face.first_point as usize;
    let &[a, b, c] = map.points.get(first..first + 3)? else {
        return None;
    };
    let points = [a, b, c].map(wide);
    let across = cross(
        add_scaled(points[1], points[0], -1.0),
        add_scaled(points[2], points[0], -1.0),
    );
    let area = dot(across, across).sqrt() * 0.5;
    let facing = normalized(across)?;
    let normal_at = |corner: usize| {
        let stored = map.normals.get(first + corner).copied().unwrap_or([0; 3]);
        normalized(unpacked(stored)).unwrap_or(facing)
    };
    Some(Triangle {
        face: number,
        points,
        normals: [normal_at(0), normal_at(1), normal_at(2)],
        facing,
        area,
    })
}

/// A corner's place, as a key. Corners meet if they're within a 64th of a unit.
#[expect(clippy::cast_possible_truncation, reason = "worlds are small")]
fn place_key(point: Point) -> [i64; 3] {
    point.map(|part| (part * 64.0).round() as i64)
}

/// Each triangle's neighbours, across its edges.
fn neighbours(triangles: &[Triangle]) -> Vec<Vec<usize>> {
    let mut edges: HashMap<([i64; 3], [i64; 3]), Vec<usize>> = HashMap::new();
    for (number, triangle) in triangles.iter().enumerate() {
        let keys = triangle.points.map(place_key);
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let edge = if keys[a] < keys[b] {
                (keys[a], keys[b])
            } else {
                (keys[b], keys[a])
            };
            edges.entry(edge).or_default().push(number);
        }
    }
    let mut out = vec![Vec::new(); triangles.len()];
    for meeting in edges.values() {
        for one in meeting {
            for other in meeting {
                if one != other {
                    out[*one].push(*other);
                }
            }
        }
    }
    out
}

/// Two directions that lie on the plane facing `normal`.
fn plane_axes(normal: Point) -> (Point, Point) {
    let smallest = (0..3)
        .min_by(|a, b| normal[*a].abs().total_cmp(&normal[*b].abs()))
        .unwrap_or(0);
    let mut other = [0.0; 3];
    other[smallest] = 1.0;
    let across = normalized(cross(other, normal)).unwrap_or([1.0, 0.0, 0.0]);
    (across, cross(normal, across))
}

/// `triangle` on the plane of `axes`.
fn flatten(triangle: &Triangle, axes: (Point, Point)) -> Flat {
    triangle
        .points
        .map(|point| [dot(point, axes.0), dot(point, axes.1)])
}

/// Which side of `a` to `b` the point `p` is on.
fn side(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// The low and high ends of `triangle` along `axis`.
fn along(triangle: &Flat, axis: [f64; 2]) -> (f64, f64) {
    triangle
        .iter()
        .map(|point| point[0] * axis[0] + point[1] * axis[1])
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| {
            (low.min(value), high.max(value))
        })
}

/// Whether two flat triangles overlap by more than touching. Any line that
/// separates convex shapes runs along an edge of one.
fn overlap(one: &Flat, other: &Flat) -> bool {
    for shape in [one, other] {
        for (a, b) in [(0, 1), (1, 2), (2, 0)] {
            let axis = [shape[a][1] - shape[b][1], shape[b][0] - shape[a][0]];
            let (low, high) = along(one, axis);
            let (other_low, other_high) = along(other, axis);
            let slack = 1e-6 * (axis[0].abs() + axis[1].abs());
            if high <= other_low + slack || other_high <= low + slack {
                return false;
            }
        }
    }
    true
}

/// A chart as it grows.
struct Growing {
    /// Which way its first triangle faces.
    normal: Point,
    /// Its plane.
    axes: (Point, Point),
    /// What's in it, by number.
    members: Vec<usize>,
    /// Those on its plane.
    flats: Vec<Flat>,
    /// The low end of what it covers on its plane.
    low: [f64; 2],
    /// The high end.
    high: [f64; 2],
}

impl Growing {
    /// A chart of one triangle.
    fn of(number: usize, triangle: &Triangle) -> Self {
        let axes = plane_axes(triangle.facing);
        let flat = flatten(triangle, axes);
        let mut chart = Self {
            normal: triangle.facing,
            axes,
            members: vec![number],
            flats: vec![flat],
            low: [f64::INFINITY; 2],
            high: [f64::NEG_INFINITY; 2],
        };
        chart.cover(&flat);
        chart
    }

    /// Takes in what `flat` covers.
    fn cover(&mut self, flat: &Flat) {
        for point in flat {
            for (axis, part) in point.iter().enumerate() {
                self.low[axis] = self.low[axis].min(*part);
                self.high[axis] = self.high[axis].max(*part);
            }
        }
    }

    /// Takes `triangle` in, if it can lie flat here without folding over what's
    /// there or making the chart too big.
    fn take(&mut self, number: usize, triangle: &Triangle, spacing: f64) -> bool {
        if dot(triangle.facing, self.normal) < CHART_TURN || self.members.len() >= CHART_TRIANGLES {
            return false;
        }
        let flat = flatten(triangle, self.axes);
        let wide_enough = (0..2).all(|axis| {
            let low = flat.iter().fold(self.low[axis], |low, p| low.min(p[axis]));
            let high = flat
                .iter()
                .fold(self.high[axis], |high, p| high.max(p[axis]));
            (high - low) / spacing <= CHART_SAMPLES
        });
        if !wide_enough || self.flats.iter().any(|known| overlap(known, &flat)) {
            return false;
        }
        self.members.push(number);
        self.flats.push(flat);
        self.cover(&flat);
        true
    }
}

/// Where a place on a flat triangle is, as a share of each corner, held to the
/// triangle, and how far it had to move to get there.
fn share_of(flat: &Flat, place: [f64; 2]) -> ([f64; 3], f64) {
    let [a, b, c] = *flat;
    let whole = side(a, b, c);
    if whole.abs() < 1e-12 {
        return ([1.0, 0.0, 0.0], f64::INFINITY);
    }
    let raw = [
        side(b, c, place) / whole,
        side(c, a, place) / whole,
        side(a, b, place) / whole,
    ];
    let held = raw.map(|part| part.max(0.0));
    let total = held[0] + held[1] + held[2];
    let share = held.map(|part| part / total.max(1e-12));
    let at = [
        share[0] * a[0] + share[1] * b[0] + share[2] * c[0],
        share[0] * a[1] + share[1] * b[1] + share[2] * c[1],
    ];
    let gap = (at[0] - place[0]).hypot(at[1] - place[1]);
    (share, gap)
}

/// The surface of `triangle` at `share`.
fn spot_on(triangle: &Triangle, share: [f64; 3]) -> Spot {
    let mut point = [0.0; 3];
    let mut normal = [0.0; 3];
    for (corner, part) in share.iter().enumerate() {
        point = add_scaled(point, triangle.points[corner], *part);
        normal = add_scaled(normal, triangle.normals[corner], *part);
    }
    let normal = normalized(normal).unwrap_or(triangle.facing);
    Spot {
        point: add_scaled(point, normal, SAMPLE_LIFT),
        normal,
    }
}

/// A map axis, from a direction in full.
#[expect(clippy::cast_possible_truncation, reason = "maps store f32")]
fn map_axis(axis: Point) -> TexelAxis {
    TexelAxis {
        axis: Vec3::new(axis[0] as f32, axis[1] as f32, axis[2] as f32),
        offset: 0.0,
    }
}

/// The finished chart of `growing`.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "charts are small"
)]
fn finished(growing: &Growing, triangles: &[Triangle], spacing: f64) -> Chart {
    let origin = [0, 1].map(|axis| (growing.low[axis] / spacing).floor() * spacing);
    let count = |axis: usize| ((growing.high[axis] - origin[axis]) / spacing).ceil() as u32 + 2;
    let (width, height) = (count(0), count(1));
    let mut spots = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let place = [
                origin[0] + f64::from(x) * spacing,
                origin[1] + f64::from(y) * spacing,
            ];
            let nearest = growing
                .flats
                .iter()
                .zip(&growing.members)
                .map(|(flat, member)| (share_of(flat, place), *member))
                .min_by(|(one, _), (other, _)| one.1.total_cmp(&other.1));
            if let Some(((share, _), member)) = nearest {
                spots.push(spot_on(&triangles[member], share));
            }
        }
    }
    let members = growing.members.iter().map(|member| &triangles[*member]);
    let mut sum = [0.0; 3];
    let mut area = 0.0;
    let mut faces = Vec::with_capacity(growing.members.len());
    for triangle in members {
        sum = add_scaled(sum, triangle.facing, triangle.area);
        area += triangle.area;
        faces.push(triangle.face);
    }
    Chart {
        faces,
        axes: [map_axis(growing.axes.0), map_axis(growing.axes.1)],
        width,
        height,
        origin: origin.map(|part| part as i32),
        spots,
        normal: normalized(sum).unwrap_or(growing.normal),
        area,
        probe_reach: None,
    }
}

/// The charts of `group`: triangles side by side that face much the same way.
fn charts_of(map: &Map, group: &MeshGroup, spacing: f64) -> Vec<Chart> {
    let triangles = triangles_of(map, group);
    let beside = neighbours(&triangles);
    let mut order: Vec<usize> = (0..triangles.len()).collect();
    order.sort_by(|a, b| triangles[*b].area.total_cmp(&triangles[*a].area));
    let mut taken = vec![false; triangles.len()];
    let mut charts = Vec::new();
    for start in order {
        if taken[start] {
            continue;
        }
        taken[start] = true;
        let mut growing = Growing::of(start, &triangles[start]);
        let mut next = vec![start];
        while let Some(number) = next.pop() {
            for other in &beside[number] {
                if !taken[*other] && growing.take(*other, &triangles[*other], spacing) {
                    taken[*other] = true;
                    next.push(*other);
                }
            }
        }
        charts.push(finished(&growing, &triangles, spacing));
    }
    charts
}

/// The one chart of a mesh lit by a probe: a single sample in the middle of it,
/// with axes that hold every corner to that sample.
fn probe_chart(map: &Map, group: &MeshGroup) -> Option<Chart> {
    let triangles = triangles_of(map, group);
    let mut sum = [0.0; 3];
    let mut area = 0.0;
    for triangle in &triangles {
        for point in triangle.points {
            sum = add_scaled(sum, point, triangle.area / 3.0);
        }
        area += triangle.area;
    }
    if area <= 0.0 {
        return None;
    }
    let middle = sum.map(|part| part / area);
    let far = |point: &Point| {
        let gap = add_scaled(*point, middle, -1.0);
        dot(gap, gap).sqrt()
    };
    let corners = triangles.iter().flat_map(|triangle| &triangle.points);
    let reach = corners.map(far).fold(0.0, f64::max) + PROBE_CLEARANCE;
    let none = TexelAxis {
        axis: Vec3::ZERO,
        offset: 0.0,
    };
    Some(Chart {
        faces: triangles.iter().map(|triangle| triangle.face).collect(),
        axes: [none, none],
        width: 1,
        height: 1,
        origin: [0, 0],
        spots: vec![Spot {
            point: middle,
            normal: [0.0, 0.0, 1.0],
        }],
        normal: [0.0, 0.0, 1.0],
        area,
        probe_reach: Some(reach),
    })
}

/// How far off the land a chunk's samples are, so that they aren't under it.
const OFF_THE_LAND: f64 = 2.0;

/// The chart of the chunk at `x`, `y` of `sheet`, with its samples lifted onto the land, each
/// facing the way the land does there.
fn chunk_chart(sheet: &TerrainSheet, (x, y): (u32, u32), spacing: f64) -> Chart {
    let [across, _] = sheet.data.chunks();
    #[expect(clippy::cast_possible_truncation, reason = "land is in f32")]
    let step = spacing as f32;
    let cell = sheet.data.cell;
    #[expect(clippy::cast_precision_loss, reason = "a land's size")]
    let side = CHUNK_CELLS as f32 * cell;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a count of samples"
    )]
    let samples = (side / step).ceil() as u32 + 1;
    let low = sheet.sample(x * CHUNK_CELLS, y * CHUNK_CELLS);
    let mut spots = Vec::with_capacity((samples * samples) as usize);
    for down in 0..samples {
        for along in 0..samples {
            #[expect(clippy::cast_precision_loss, reason = "a count of samples")]
            let place = (low.x + along as f32 * step, low.y + down as f32 * step);
            let (height, normal) = sheet.surface_at(place.0, place.1);
            let normal = wide(normal);
            let point = wide(Vec3::new(place.0, place.1, height));
            spots.push(Spot {
                point: [0, 1, 2].map(|axis| point[axis] + normal[axis] * OFF_THE_LAND),
                normal,
            });
        }
    }
    let axis = |axis: Vec3| TexelAxis { axis, offset: 0.0 };
    #[expect(clippy::cast_possible_truncation, reason = "a world place")]
    let origin = [low.x as i32, low.y as i32];
    Chart {
        faces: vec![sheet.first_face + y * across + x],
        axes: [
            axis(Vec3::new(1.0, 0.0, 0.0)),
            axis(Vec3::new(0.0, 1.0, 0.0)),
        ],
        width: samples,
        height: samples,
        origin,
        spots,
        normal: [0.0, 0.0, if sheet.facing_down { -1.0 } else { 1.0 }],
        area: f64::from(side) * f64::from(side),
        probe_reach: None,
    }
}

/// Every chart in `map`.
pub(crate) fn map_charts(map: &Map, spacing: f64) -> Vec<Chart> {
    let meshes = map.mesh_groups.iter().flat_map(|group| {
        if group.by_probe {
            probe_chart(map, group).into_iter().collect()
        } else {
            charts_of(map, group, spacing)
        }
    });
    let land = map.terrain.iter().flat_map(|sheet| {
        let [across, down] = sheet.data.chunks();
        (0..down).flat_map(move |y| (0..across).map(move |x| chunk_chart(sheet, (x, y), spacing)))
    });
    meshes.chain(land).collect()
}
