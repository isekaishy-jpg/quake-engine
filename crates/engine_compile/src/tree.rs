//! Trees of solid and open space. Space is first halved until each part holds
//! few brushes, so that a brush only meets its neighbours. Then in each part
//! brushes go in one at a time. Nothing is split: a brush goes down whichever
//! sides it reaches, and a chain of its planes at each leaf marks what's inside
//! it.

use std::collections::HashMap;

use engine_map::Contents;

use crate::geometry::{ON_PLANE, Plane3, Point};

/// A node's child.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Child {
    /// Another node.
    Node(u32),
    /// A region of `Contents`.
    Leaf(Contents),
}

/// A node, dividing space by a plane.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BuildNode {
    /// The plane, by index.
    pub(crate) plane: u32,
    /// In front of the plane.
    pub(crate) front: Child,
    /// Behind it.
    pub(crate) back: Child,
}

/// How many brushes a part of space holds before it's halved again.
const BRUSHES_PER_PART: usize = 8;

/// How many times space is halved, at most.
const MOST_HALVINGS: u32 = 28;

/// Planes, kept once each.
#[derive(Debug, Default)]
pub(crate) struct PlaneSet {
    /// The planes.
    pub(crate) planes: Vec<Plane3>,
    /// Planes by their distance, to the nearest unit, to find them quickly.
    by_distance: HashMap<i64, Vec<u32>>,
}

/// A brush going into a tree.
#[derive(Clone, Debug)]
pub(crate) struct Item {
    /// Its planes, by index.
    pub(crate) planes: Vec<u32>,
    /// Its corners.
    pub(crate) corners: Vec<Point>,
    /// What fills it.
    pub(crate) contents: Contents,
}

impl Item {
    /// How far it reaches along `axis`.
    fn extent(&self, axis: usize) -> (f64, f64) {
        self.corners
            .iter()
            .fold((f64::MAX, f64::MIN), |(low, high), corner| {
                (low.min(corner[axis]), high.max(corner[axis]))
            })
    }
}

impl PlaneSet {
    /// The index of `plane`, adding it if it's new.
    #[expect(clippy::cast_possible_truncation, reason = "distances are small")]
    pub(crate) fn index_of(&mut self, plane: Plane3) -> u32 {
        let bucket = plane.distance.round() as i64;
        let same = |known: &Plane3| {
            (known.distance - plane.distance).abs() < ON_PLANE
                && (0..3).all(|axis| (known.normal[axis] - plane.normal[axis]).abs() < ON_PLANE)
        };
        for near in bucket - 1..=bucket + 1 {
            let candidates = self.by_distance.get(&near).into_iter().flatten();
            for index in candidates {
                if same(&self.planes[*index as usize]) {
                    return *index;
                }
            }
        }
        let index = u32::try_from(self.planes.len()).unwrap_or(u32::MAX);
        self.planes.push(plane);
        self.by_distance.entry(bucket).or_default().push(index);
        index
    }
}

/// What fills a place `new` was added to, given it held `old`.
fn merged(old: Contents, new: Contents) -> Contents {
    if old == Contents::Solid || new == Contents::Solid {
        Contents::Solid
    } else {
        new
    }
}

/// The best way to halve `chosen`: an axis, a place along it, and what's each
/// side, or `None` if no cut leaves both sides with less.
#[expect(clippy::type_complexity, reason = "used once, just below")]
fn halve(items: &[Item], chosen: &[usize]) -> Option<(usize, f64, Vec<usize>, Vec<usize>)> {
    let mut best: Option<(usize, usize, f64, Vec<usize>, Vec<usize>)> = None;
    for axis in 0..3 {
        let mut middles: Vec<f64> = chosen
            .iter()
            .map(|at| {
                let (low, high) = items[*at].extent(axis);
                (low + high) * 0.5
            })
            .collect();
        middles.sort_by(f64::total_cmp);
        let place = middles[middles.len() / 2].round();
        let reaches = |at: &&usize| items[**at].extent(axis);
        let low: Vec<usize> = chosen
            .iter()
            .filter(|at| reaches(at).0 < place - ON_PLANE)
            .copied()
            .collect();
        let high: Vec<usize> = chosen
            .iter()
            .filter(|at| reaches(at).1 > place + ON_PLANE)
            .copied()
            .collect();
        if low.len() >= chosen.len() || high.len() >= chosen.len() {
            continue;
        }
        let cost = low.len().max(high.len()) * 2 + low.len() + high.len();
        if best.as_ref().is_none_or(|found| cost < found.1) {
            best = Some((axis, cost, place, low, high));
        }
    }
    best.map(|(axis, _, place, low, high)| (axis, place, low, high))
}

/// A tree under construction.
#[derive(Debug, Default)]
pub(crate) struct TreeBuilder {
    /// The nodes.
    pub(crate) nodes: Vec<BuildNode>,
}

impl TreeBuilder {
    /// Adds a node.
    fn push(&mut self, node: BuildNode) -> Child {
        self.nodes.push(node);
        Child::Node(u32::try_from(self.nodes.len() - 1).unwrap_or(u32::MAX))
    }

    /// A chain marking what's behind all `planes` as `contents`, and the rest
    /// as `outside`.
    fn chain(&mut self, outside: Contents, planes: &[u32], contents: Contents) -> Child {
        if outside == Contents::Solid {
            return Child::Leaf(Contents::Solid);
        }
        let mut child = Child::Leaf(merged(outside, contents));
        for plane in planes.iter().rev() {
            child = self.push(BuildNode {
                plane: *plane,
                front: Child::Leaf(outside),
                back: child,
            });
        }
        child
    }

    /// A tree of `items`, keeping their order wherever they overlap.
    pub(crate) fn build(&mut self, set: &mut PlaneSet, items: &[Item]) -> Child {
        let all: Vec<usize> = (0..items.len()).collect();
        self.build_part(set, items, &all, MOST_HALVINGS)
    }

    /// A tree of the `chosen` items, halving space while it helps.
    fn build_part(
        &mut self,
        set: &mut PlaneSet,
        items: &[Item],
        chosen: &[usize],
        halvings: u32,
    ) -> Child {
        let halved = (chosen.len() > BRUSHES_PER_PART && halvings > 0)
            .then(|| halve(items, chosen))
            .flatten();
        let Some((axis, place, low, high)) = halved else {
            let mut child = Child::Leaf(Contents::Empty);
            for at in chosen {
                let item = &items[*at];
                child = self.insert(child, set, &item.planes, &item.corners, item.contents);
            }
            return child;
        };
        let mut normal = [0.0; 3];
        normal[axis] = 1.0;
        let plane = set.index_of(Plane3 {
            normal,
            distance: place,
        });
        let front = self.build_part(set, items, &high, halvings - 1);
        let back = self.build_part(set, items, &low, halvings - 1);
        self.push(BuildNode { plane, front, back })
    }

    /// Inserts a brush under `child`, and returns what replaces it. `corners`
    /// says which sides of a node the brush reaches.
    pub(crate) fn insert(
        &mut self,
        child: Child,
        set: &PlaneSet,
        planes: &[u32],
        corners: &[Point],
        contents: Contents,
    ) -> Child {
        let at = match child {
            Child::Leaf(outside) => return self.chain(outside, planes, contents),
            Child::Node(at) => at as usize,
        };
        let node = self.nodes[at];
        let plane = set.planes[node.plane as usize];
        let sides = corners.iter().map(|corner| plane.side(*corner));
        let (mut front, mut back) = (false, false);
        for side in sides {
            front |= side > ON_PLANE;
            back |= side < -ON_PLANE;
        }
        if front {
            let new = self.insert(node.front, set, planes, corners, contents);
            self.nodes[at].front = new;
        }
        if back {
            let new = self.insert(node.back, set, planes, corners, contents);
            self.nodes[at].back = new;
        }
        child
    }
}
