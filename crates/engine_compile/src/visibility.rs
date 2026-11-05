//! Working out which cells can see each other (proposal 0088).

use std::ops::Not;

use engine_core::math::Vec3;
use engine_map::{Contents, Map};

/// Where each cell's sample points sit, as shares of its size.
const SAMPLE_PLACES: [[f32; 3]; 9] = [
    [0.5, 0.5, 0.5],
    [0.15, 0.15, 0.15],
    [0.85, 0.15, 0.15],
    [0.15, 0.85, 0.15],
    [0.85, 0.85, 0.15],
    [0.15, 0.15, 0.85],
    [0.85, 0.15, 0.85],
    [0.15, 0.85, 0.85],
    [0.85, 0.85, 0.85],
];

/// Points just off the faces a leaf lists, inside it. A thin space can pass
/// through a cell between fixed points, but it has surfaces, and they're what
/// there is to see.
fn face_points(map: &Map, leaf: usize, low: Vec3, high: Vec3) -> Vec<Vec3> {
    let Some(found) = map.tree.leaves.get(leaf) else {
        return Vec::new();
    };
    let first = found.first_face as usize;
    let listed = map
        .tree
        .leaf_faces
        .get(first..first + found.face_count as usize)
        .unwrap_or_default();
    let step = (listed.len() / MAX_FACE_SAMPLES).max(1);
    let mut out = Vec::new();
    for face in listed.iter().step_by(step) {
        let Some(face) = map.faces.get(*face as usize) else {
            continue;
        };
        let points = map.face_points(face);
        let [a, b, c, ..] = points else {
            continue;
        };
        let normal = Vec3::new(
            (b.y - a.y) * (c.z - a.z) - (b.z - a.z) * (c.y - a.y),
            (b.z - a.z) * (c.x - a.x) - (b.x - a.x) * (c.z - a.z),
            (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x),
        )
        .normalized();
        #[expect(clippy::cast_precision_loss, reason = "faces have few corners")]
        let share = 1.0 / points.len() as f32;
        let middle = points.iter().fold(Vec3::ZERO, |sum, at| sum + *at * share);
        let inside = Vec3::new(
            middle.x.clamp(low.x, high.x),
            middle.y.clamp(low.y, high.y),
            middle.z.clamp(low.z, high.z),
        );
        out.push(inside + normal * FACE_LIFT);
        out.push(inside - normal * FACE_LIFT);
    }
    out
}

/// A leaf's sample points that aren't in anything solid.
fn sample_points(map: &Map, leaf: usize) -> Vec<Vec3> {
    let Some((low, high)) = map.tree.leaf_boxes.get(leaf).copied() else {
        return Vec::new();
    };
    let size = high - low;
    let open = |point: &Vec3| {
        map.tree
            .leaf(map.tree.leaf_at(*point))
            .is_some_and(|found| found.contents != Contents::Solid)
    };
    let fixed = SAMPLE_PLACES
        .iter()
        .map(|[x, y, z]| low + Vec3::new(size.x * x, size.y * y, size.z * z));
    fixed
        .chain(face_points(map, leaf, low, high))
        .filter(open)
        .collect()
}

/// How many of a leaf's faces are sampled.
const MAX_FACE_SAMPLES: usize = 12;

/// How far off a face its sample sits.
const FACE_LIFT: f32 = 12.0;

/// Whether two boxes touch or overlap.
fn neighbours(a: (Vec3, Vec3), b: (Vec3, Vec3)) -> bool {
    let reach = 1.0;
    a.0.x <= b.1.x + reach
        && b.0.x <= a.1.x + reach
        && a.0.y <= b.1.y + reach
        && b.0.y <= a.1.y + reach
        && a.0.z <= b.1.z + reach
        && b.0.z <= a.1.z + reach
}

/// A set of leaves as `Tree::visibility` stores it: a bit for each from leaf 1,
/// with a run of empty bytes as a zero and a count.
fn encode(seen: &[bool]) -> Vec<u8> {
    let bytes: Vec<u8> = seen[1..]
        .chunks(8)
        .map(|bits| {
            bits.iter()
                .enumerate()
                .fold(0, |byte, (bit, on)| byte | (u8::from(*on) << bit))
        })
        .collect();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != 0 {
            out.push(bytes[at]);
            at += 1;
            continue;
        }
        let run = bytes[at..]
            .iter()
            .take(255)
            .take_while(|byte| **byte == 0)
            .count();
        out.push(0);
        out.push(u8::try_from(run).unwrap_or(u8::MAX));
        at += run;
    }
    out
}

/// Fills in `map`'s visible sets. `clear` says whether a line is free of what
/// blocks sight.
pub(crate) fn bake_visibility(map: &mut Map, clear: &(dyn Fn(Vec3, Vec3) -> bool + Sync)) {
    let count = map.tree.leaves.len();
    let samples: Vec<Vec<Vec3>> = (0..count).map(|leaf| sample_points(map, leaf)).collect();
    let spatial: Vec<usize> = (1..count)
        .filter(|leaf| !samples[*leaf].is_empty())
        .collect();
    let mut sees = vec![vec![false; count]; count];
    let rows = crate::light::in_parallel(&spatial, |from| {
        let mut row = vec![false; count];
        row[*from] = true;
        for to in spatial.iter().filter(|to| *to > from) {
            row[*to] = samples[*from]
                .iter()
                .any(|a| samples[*to].iter().any(|b| clear(*a, *b)));
        }
        row
    });
    for (from, row) in spatial.iter().zip(rows) {
        for (to, seen) in row.iter().enumerate() {
            if *seen {
                sees[*from][to] = true;
                sees[to][*from] = true;
            }
        }
    }
    // One cell wider, so a thin gap the samples missed can't show a hole.
    let boxes = &map.tree.leaf_boxes;
    let around: Vec<Vec<usize>> = (0..count)
        .map(|leaf| {
            let others = spatial.iter().copied();
            others
                .filter(|other| {
                    samples[leaf].is_empty().not() && neighbours(boxes[leaf], boxes[*other])
                })
                .collect()
        })
        .collect();
    let mut wide = sees.clone();
    for from in &spatial {
        for to in &spatial {
            if sees[*from][*to] {
                for next in &around[*to] {
                    wide[*from][*next] = true;
                }
            }
        }
    }
    let mut data = Vec::new();
    for (leaf, seen) in map.tree.leaves.iter_mut().zip(&wide).skip(1) {
        leaf.visibility = u32::try_from(data.len()).ok();
        data.extend(encode(seen));
    }
    map.tree.visible_leaf_count = u32::try_from(count.saturating_sub(1)).unwrap_or(0);
    map.tree.visibility = data;
}
