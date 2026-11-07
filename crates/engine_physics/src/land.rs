//! Collision with land, straight from its heights (proposal 0144). Hulls can't hold anything so
//! fine, so a box is swept against the triangles of the cells it crosses. A surface only stops what
//! goes into it from the side it faces, which is what lets sheets stack.

use engine_core::math::Vec3;
use engine_map::{Map, TerrainSheet};

/// How far back from land a box stops, as the hulls do, so that what's resting isn't touching.
const LAND_EPSILON: f32 = 0.03125;

/// A box going somewhere.
#[derive(Clone, Copy, Debug)]
struct Sweep {
    /// Where its middle starts.
    from: Vec3,
    /// How far its middle goes.
    way: Vec3,
    /// Half its size each way.
    half: Vec3,
}

impl Sweep {
    /// How far a box this size reaches along `axis`.
    fn reach(&self, axis: Vec3) -> f32 {
        axis.x.abs() * self.half.x + axis.y.abs() * self.half.y + axis.z.abs() * self.half.z
    }
}

/// The range of the shares of a sweep over which it overlaps a triangle, so far.
#[derive(Clone, Copy, Debug)]
struct Overlap {
    /// When it first overlaps on every axis tested.
    enter: f32,
    /// When it last does.
    leave: f32,
    /// The axis it came in along.
    axis: Vec3,
}

impl Overlap {
    /// Narrows this by `axis`. `false` is that the box never meets the triangle.
    fn narrow(&mut self, sweep: &Sweep, triangle: &[Vec3; 3], axis: Vec3) -> bool {
        if axis.dot(axis) < 1e-10 {
            return true;
        }
        let [a, b, c] = triangle.map(|corner| corner.dot(axis));
        let (low, high) = (a.min(b).min(c), a.max(b).max(c));
        let reach = sweep.reach(axis);
        let centre = sweep.from.dot(axis);
        let speed = sweep.way.dot(axis);
        // Its overlap along this axis is from when its near side reaches `low` or `high`.
        let (first, last) = (low - reach - centre, high + reach - centre);
        if speed.abs() < 1e-9 {
            return first <= 0.0 && last >= 0.0;
        }
        let (enter, leave) = if speed > 0.0 {
            (first / speed, last / speed)
        } else {
            (last / speed, first / speed)
        };
        if enter > self.enter {
            self.enter = enter;
            self.axis = axis;
        }
        self.leave = self.leave.min(leave);
        self.enter <= self.leave
    }
}

/// When `sweep` first meets `triangle` going into its face, and which way that pushes it. It's the
/// separating axes of a box and a triangle, by when each stops separating them.
fn sweep_triangle(sweep: &Sweep, triangle: &[Vec3; 3]) -> Option<(f32, Vec3)> {
    let edges = [
        triangle[1] - triangle[0],
        triangle[2] - triangle[1],
        triangle[0] - triangle[2],
    ];
    let face = edges[0].cross(edges[1]);
    if sweep.way.dot(face) >= 0.0 {
        return None;
    }
    let mut overlap = Overlap {
        enter: f32::MIN,
        leave: f32::MAX,
        axis: Vec3::ZERO,
    };
    let axes = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    ];
    if !overlap.narrow(sweep, triangle, face) {
        return None;
    }
    for axis in axes {
        if !overlap.narrow(sweep, triangle, axis) {
            return None;
        }
    }
    for edge in edges {
        for axis in axes {
            if !overlap.narrow(sweep, triangle, edge.cross(axis)) {
                return None;
            }
        }
    }
    if overlap.enter > 1.0 || overlap.leave < 0.0 {
        return None;
    }
    // What's slid along a slope comes in along its face, even if a corner met first, or a box
    // would catch on every seam between triangles.
    let facing = face * (1.0 / face.length().max(f32::EPSILON));
    Some((overlap.enter.max(0.0), facing))
}

/// The cells of `sheet` a box could touch over a stretch of its way, from `low` to `high`.
fn cells_in(sheet: &TerrainSheet, low: Vec3, high: Vec3) -> Option<[u32; 4]> {
    let cell = sheet.data.cell;
    let cells = sheet.data.cells;
    let from = |value: f32, origin: f32| ((value - origin) / cell).floor();
    let (first_x, last_x) = (from(low.x, sheet.origin.x), from(high.x, sheet.origin.x));
    let (first_y, last_y) = (from(low.y, sheet.origin.y), from(high.y, sheet.origin.y));
    #[expect(clippy::cast_precision_loss, reason = "a land's size")]
    if last_x < 0.0 || last_y < 0.0 || first_x >= cells[0] as f32 || first_y >= cells[1] as f32 {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "kept within the sheet"
    )]
    let at = |value: f32, count: u32| value.max(0.0).min((count - 1) as f32) as u32;
    Some([
        at(first_x, cells[0]),
        at(first_y, cells[1]),
        at(last_x, cells[0]),
        at(last_y, cells[1]),
    ])
}

/// The first hit of `sweep` on `sheet`, within what `best` leaves.
fn sweep_sheet(sheet: &TerrainSheet, sweep: &Sweep, best: &mut Option<(f32, Vec3)>) {
    let cell = sheet.data.cell.max(1.0);
    let length = sweep.way.length();
    // It goes a cell at a time, so a long line only looks near itself.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a count of steps"
    )]
    let steps = (length / cell).ceil().max(1.0) as u32;
    #[expect(clippy::cast_precision_loss, reason = "a count of steps")]
    let step = 1.0 / steps as f32;
    for at in 0..steps {
        #[expect(clippy::cast_precision_loss, reason = "a count of steps")]
        let start = at as f32 * step;
        if best.is_some_and(|(fraction, _)| fraction < start) {
            return;
        }
        let (one, two) = (
            sweep.from + sweep.way * start,
            sweep.from + sweep.way * (start + step),
        );
        let low = Vec3::new(one.x.min(two.x), one.y.min(two.y), one.z.min(two.z)) - sweep.half;
        let high = Vec3::new(one.x.max(two.x), one.y.max(two.y), one.z.max(two.z)) + sweep.half;
        let Some([first_x, first_y, last_x, last_y]) = cells_in(sheet, low, high) else {
            continue;
        };
        for y in first_y..=last_y {
            for x in first_x..=last_x {
                if sheet.data.is_hole(x, y) {
                    continue;
                }
                for triangle in sheet.cell_triangles(x, y) {
                    let Some((fraction, normal)) = sweep_triangle(sweep, &triangle) else {
                        continue;
                    };
                    if best.is_none_or(|(nearest, _)| fraction < nearest) {
                        *best = Some((fraction, normal));
                    }
                }
            }
        }
    }
}

/// How far a box from `start` to `end` gets before it meets land, and which way that faces, with
/// the share taken back so that it stops just short.
#[must_use]
pub(crate) fn sweep_land(
    map: &Map,
    start: Vec3,
    (mins, maxs): (Vec3, Vec3),
    end: Vec3,
) -> Option<(f32, Vec3)> {
    if map.terrain.is_empty() {
        return None;
    }
    let middle = (mins + maxs) * 0.5;
    let sweep = Sweep {
        from: start + middle,
        way: end - start,
        half: (maxs - mins) * 0.5,
    };
    let mut best = None;
    for sheet in &map.terrain {
        sweep_sheet(sheet, &sweep, &mut best);
    }
    let length = sweep.way.length().max(f32::EPSILON);
    best.map(|(fraction, normal): (f32, Vec3)| {
        // Back along the way it came, by enough to stand clear of the face.
        let into = -normal.dot(sweep.way) / length;
        let back = LAND_EPSILON / into.max(0.01) / length;
        ((fraction - back).max(0.0), normal)
    })
}

/// Whether `point` is inside land.
#[must_use]
pub(crate) fn in_land(map: &Map, point: Vec3) -> bool {
    map.terrain.iter().any(|sheet| sheet.holds(point))
}
