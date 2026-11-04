//! Moving boxes through clipping hulls, from `SV_RecursiveHullCheck` in
//! `world.c`. The player's own moves use `box3d`, and this serves everything
//! else: points use the drawing tree, and boxes the hulls grown to fit them.

use engine_core::math::Vec3;
use engine_map::{ClipChild, Contents, NodeChild, Plane, Tree};

/// How far short of a plane a move stops.
const DIST_EPSILON: f32 = 0.031_25;

/// Where a hull begins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HullRoot {
    /// The drawing tree, for points.
    Point(NodeChild),
    /// A clipping hull, for boxes.
    Clip(ClipChild),
}

/// A move through a hull.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct HullTrace {
    /// How far it got, from 0 to 1.
    pub(crate) fraction: f32,
    /// Where it stopped.
    pub(crate) end: Vec3,
    /// The plane it hit.
    pub(crate) normal: Vec3,
    /// Whether it began in solid.
    pub(crate) start_solid: bool,
    /// Whether it never left solid.
    pub(crate) all_solid: bool,
    /// Whether it passed through open air.
    pub(crate) in_open: bool,
    /// Whether it passed through liquid.
    pub(crate) in_water: bool,
}

/// What a step down a hull finds.
enum Step {
    /// Open space, and whether it's liquid.
    Open(bool),
    /// Solid.
    Solid,
    /// A plane, with what's in front and behind.
    Split(Plane, [HullRoot; 2]),
}

/// Looks at `root`. Something missing from a damaged map counts as solid.
fn step(tree: &Tree, root: HullRoot) -> Step {
    let usize_of = |value: u32| usize::try_from(value).unwrap_or(usize::MAX);
    match root {
        HullRoot::Point(NodeChild::Leaf(leaf)) => match tree.leaf(leaf) {
            Some(leaf) if leaf.contents != Contents::Solid => {
                Step::Open(leaf.contents != Contents::Empty)
            }
            _ => Step::Solid,
        },
        HullRoot::Point(NodeChild::Node(index)) => tree
            .nodes
            .get(usize_of(index))
            .and_then(|node| {
                let plane = *tree.planes.get(usize_of(node.plane))?;
                let children = node.children.map(HullRoot::Point);
                Some(Step::Split(plane, children))
            })
            .unwrap_or(Step::Solid),
        HullRoot::Clip(ClipChild::Empty) => Step::Open(false),
        HullRoot::Clip(ClipChild::Solid) => Step::Solid,
        HullRoot::Clip(ClipChild::Node(index)) => tree
            .clip_nodes
            .get(usize_of(index))
            .and_then(|node| {
                let plane = *tree.planes.get(usize_of(node.plane))?;
                let children = node.children.map(HullRoot::Clip);
                Some(Step::Split(plane, children))
            })
            .unwrap_or(Step::Solid),
    }
}

/// `SV_HullPointContents`: whether `point` is in solid.
fn solid_at(tree: &Tree, mut root: HullRoot, point: Vec3) -> bool {
    loop {
        match step(tree, root) {
            Step::Open(_) => return false,
            Step::Solid => return true,
            Step::Split(plane, children) => {
                let side = plane.normal.dot(point) - plane.distance;
                root = children[usize::from(side < 0.0)];
            }
        }
    }
}

/// `SV_RecursiveHullCheck`. Returns whether the move went on, so `false` means
/// it stopped.
fn check(
    tree: &Tree,
    top: HullRoot,
    root: HullRoot,
    from: (f32, Vec3),
    to: (f32, Vec3),
    trace: &mut HullTrace,
) -> bool {
    let (plane, children) = match step(tree, root) {
        Step::Open(wet) => {
            trace.all_solid = false;
            trace.in_water |= wet;
            trace.in_open |= !wet;
            return true;
        }
        Step::Solid => {
            trace.start_solid = true;
            return true;
        }
        Step::Split(plane, children) => (plane, children),
    };
    let start = plane.normal.dot(from.1) - plane.distance;
    let end = plane.normal.dot(to.1) - plane.distance;
    if start >= 0.0 && end >= 0.0 {
        return check(tree, top, children[0], from, to, trace);
    }
    if start < 0.0 && end < 0.0 {
        return check(tree, top, children[1], from, to, trace);
    }
    // Put the crossing a little on the near side.
    let nudge = if start < 0.0 {
        DIST_EPSILON
    } else {
        -DIST_EPSILON
    };
    let mut fraction = ((start + nudge) / (start - end)).clamp(0.0, 1.0);
    let at = |fraction: f32| {
        (
            from.0 + (to.0 - from.0) * fraction,
            from.1.lerp(to.1, fraction),
        )
    };
    let mut middle = at(fraction);
    let side = usize::from(start < 0.0);
    if !check(tree, top, children[side], from, middle, trace) {
        return false;
    }
    if !solid_at(tree, children[1 - side], middle.1) {
        return check(tree, top, children[1 - side], middle, to, trace);
    }
    if trace.all_solid {
        return false;
    }
    trace.normal = if side == 0 {
        plane.normal
    } else {
        plane.normal * -1.0
    };
    while solid_at(tree, top, middle.1) {
        fraction -= 0.1;
        if fraction < 0.0 {
            break;
        }
        middle = at(fraction);
    }
    trace.fraction = middle.0;
    trace.end = middle.1;
    false
}

/// Moves a point from `start` to `end` through the hull at `root`.
pub(crate) fn trace_hull(tree: &Tree, root: HullRoot, start: Vec3, end: Vec3) -> HullTrace {
    let mut trace = HullTrace {
        fraction: 1.0,
        end,
        normal: Vec3::ZERO,
        start_solid: false,
        all_solid: true,
        in_open: false,
        in_water: false,
    };
    check(tree, root, root, (0.0, start), (1.0, end), &mut trace);
    if trace.all_solid {
        trace.start_solid = true;
    }
    trace
}

/// Where a line from `start` to `end` enters a box: how far along, the box's
/// side there, and whether it began inside.
#[must_use]
pub fn enter_box(start: Vec3, end: Vec3, low: Vec3, high: Vec3) -> Option<(f32, Vec3, bool)> {
    let starts = [start.x, start.y, start.z];
    let ends = [end.x, end.y, end.z];
    let lows = [low.x, low.y, low.z];
    let highs = [high.x, high.y, high.z];
    let axes = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    ];
    let (mut enter, mut leave) = (0.0_f32, 1.0_f32);
    let mut normal = Vec3::ZERO;
    let mut inside = true;
    for axis in 0..3 {
        let delta = ends[axis] - starts[axis];
        let outside = starts[axis] < lows[axis] || starts[axis] > highs[axis];
        inside &= !outside;
        if delta == 0.0 {
            if outside {
                return None;
            }
            continue;
        }
        let (near, far) = if delta > 0.0 {
            (lows[axis], highs[axis])
        } else {
            (highs[axis], lows[axis])
        };
        let first = (near - starts[axis]) / delta;
        if first > enter {
            enter = first;
            normal = axes[axis] * if delta > 0.0 { -1.0 } else { 1.0 };
        }
        leave = leave.min((far - starts[axis]) / delta);
    }
    if inside {
        return Some((0.0, Vec3::ZERO, true));
    }
    (enter <= leave && enter < 1.0).then_some((enter, normal, false))
}
