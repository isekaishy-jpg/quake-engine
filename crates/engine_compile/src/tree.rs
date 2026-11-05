//! Trees of solid and open space, built by inserting one convex brush at a
//! time. Nothing is split: a brush goes down whichever sides it reaches, and a
//! chain of its planes at each leaf marks what's inside it.

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

/// Planes, kept once each.
#[derive(Debug, Default)]
pub(crate) struct PlaneSet {
    /// The planes.
    pub(crate) planes: Vec<Plane3>,
}

impl PlaneSet {
    /// The index of `plane`, adding it if it's new.
    pub(crate) fn index_of(&mut self, plane: Plane3) -> u32 {
        let found = self.planes.iter().position(|known| {
            (known.distance - plane.distance).abs() < ON_PLANE
                && (0..3).all(|axis| (known.normal[axis] - plane.normal[axis]).abs() < ON_PLANE)
        });
        let index = found.unwrap_or_else(|| {
            self.planes.push(plane);
            self.planes.len() - 1
        });
        u32::try_from(index).unwrap_or(u32::MAX)
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
